import { copyFileSync, existsSync, mkdirSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";

type Estimate = {
  median: {
    point_estimate: number;
    confidence_interval: {
      lower_bound: number;
      upper_bound: number;
    };
  };
};

type Benchmark = {
  group_id: string;
  function_id: string;
  value_str: string;
};

type Backend = "recs" | "bevy_table" | "bevy_sparse";

type ResultRow = {
  key: string;
  groupId: string;
  workload: string;
  size: string;
  results: Partial<Record<Backend, Estimate>>;
};

type ReportCell = {
  median: number;
  low: number;
  high: number;
  ratio: number;
};

type ReportRow = {
  groupId: string;
  label: string;
  size: string;
  aggregate?: boolean;
  results: Partial<Record<Backend, ReportCell | { ratio: number }>>;
};

const backends: Backend[] = ["recs", "bevy_table", "bevy_sparse"];
const backendLabels: Record<Backend, string> = {
  recs: "recs · SparseSet",
  bevy_table: "Bevy · Table",
  bevy_sparse: "Bevy · SparseSet",
};

const repoRoot = resolve(import.meta.dir, "../..");
const criterionRoot = join(repoRoot, "target", "criterion");
const reportPath = join(criterionRoot, "ecs-heatmap.html");
const reportSvgPath = join(repoRoot, "benches", "report", "heatmap.svg");
const groupOrder = [
  "query/dense_read",
  "query/dense_write",
  "query/sparse_join_read",
  "query/fragmented_read",
  "component/add_remove",
  "entity/spawn",
  "entity/despawn_spawn",
  "entity/random_get",
];
const allowedGroups = new Set(groupOrder);
const groupLabels: Record<string, string> = {
  "query/dense_read": "密集读取",
  "query/dense_write": "密集写入",
  "query/sparse_join_read": "稀疏匹配查询",
  "query/fragmented_read": "组合碎片查询",
  "component/add_remove": "组件增删",
  "entity/spawn": "实体生成",
  "entity/despawn_spawn": "实体销毁重建",
  "entity/random_get": "随机查组件",
};
const groupDescriptions: Record<string, string> = {
  "query/dense_read": "逐实体遍历并读取两项组件数据。",
  "query/dense_write": "逐实体遍历读取两项组件数据，并写入其中一项。",
  "query/sparse_join_read": "一项组件数据覆盖全部实体，另一项按匹配率稀疏分布。",
  "query/fragmented_read": "按 4 个标记形成 16 种实体组合后遍历查询。",
  "component/add_remove": "在预建实体上反复添加、移除组件。",
  "entity/spawn": "测量新建 World 和实体创建，包含 Bevy 批量创建。",
  "entity/despawn_spawn": "循环销毁全部实体，再创建同规模实体。",
  "entity/random_get": "按固定乱序访问实体并读取组件数据。",
};

function walk(directory: string): string[] {
  let paths: string[] = [];
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) {
      paths = paths.concat(walk(path));
    } else if (entry.name === "benchmark.json" && path.includes(`${join("new", "benchmark.json")}`)) {
      paths.push(path);
    }
  }
  return paths;
}

function backendFor(functionId: string): Backend | undefined {
  if (functionId.startsWith("recs_sparse")) return "recs";
  if (functionId.startsWith("bevy_table")) return "bevy_table";
  if (functionId.startsWith("bevy_sparse")) return "bevy_sparse";
  return undefined;
}

function benchmarkKey(benchmark: Benchmark): { key: string; workload: string } {
  const functionParts = benchmark.function_id.split("/");
  let scenario = benchmark.group_id;

  if (benchmark.group_id === "query/sparse_join_read") {
    scenario += `/${functionParts[1]}`;
  } else if (benchmark.group_id === "entity/spawn") {
    const mode = benchmark.function_id.endsWith("_batch") ? "batch" : "single";
    scenario += `/${mode}`;
  }

  return { key: `${scenario}/${benchmark.value_str}`, workload: scenario };
}

function naturalCompare(a: ResultRow, b: ResultRow): number {
  const groupOrderDiff = groupOrder.indexOf(a.groupId) - groupOrder.indexOf(b.groupId);
  if (groupOrderDiff !== 0) return groupOrderDiff;

  if (a.groupId === "query/sparse_join_read") {
    const aRate = Number(a.workload.match(/\/(\d+)pct$/)?.[1] ?? 0);
    const bRate = Number(b.workload.match(/\/(\d+)pct$/)?.[1] ?? 0);
    if (aRate !== bRate) return aRate - bRate;
  } else if (a.groupId === "entity/spawn") {
    const aMode = a.workload.endsWith("/single") ? 0 : 1;
    const bMode = b.workload.endsWith("/single") ? 0 : 1;
    if (aMode !== bMode) return aMode - bMode;
  }

  return Number(a.size) - Number(b.size);
}

function geometricMean(values: number[]): number | undefined {
  if (values.length === 0) return undefined;
  return Math.exp(values.reduce((sum, value) => sum + Math.log(value), 0) / values.length);
}

function workloadLabel(workload: string): string {
  if (workload.startsWith("query/sparse_join_read/")) {
    const percent = workload.slice("query/sparse_join_read/".length).match(/^(\d+)pct$/)?.[1];
    return percent ? `${percent}%` : "";
  }
  if (workload === "entity/spawn/single") return "逐个";
  if (workload === "entity/spawn/batch") return "批量";
  return "";
}

function formatEntityCount(size: string): string {
  const count = Number(size);
  if (count >= 1_000_000) return `${count / 1_000_000}M`;
  if (count >= 1_000) return `${count / 1_000}k`;
  return count.toLocaleString("zh-CN");
}

function toReportRows(rows: ResultRow[]): { rows: ReportRow[]; maxRatio: number } {
  const ratiosByBackend: Record<Backend, number[]> = {
    recs: [],
    bevy_table: [],
    bevy_sparse: [],
  };
  let maxRatio = 1;
  const reportRows: ReportRow[] = [];

  rows.forEach((row) => {
    const present = backends
      .map((backend) => row.results[backend]?.median.point_estimate)
      .filter((value): value is number => value !== undefined && value > 0);
    const fastest = Math.min(...present);
    const results: ReportRow["results"] = {};

    backends.forEach((backend) => {
      const estimate = row.results[backend];
      if (!estimate || present.length === 0) return;

      const median = estimate.median.point_estimate;
      const ratio = median / fastest;
      maxRatio = Math.max(maxRatio, ratio);
      ratiosByBackend[backend].push(ratio);
      results[backend] = {
          median,
          low: estimate.median.confidence_interval.lower_bound,
          high: estimate.median.confidence_interval.upper_bound,
        ratio,
      };
    });

    reportRows.push({
      groupId: row.groupId,
      label: workloadLabel(row.workload),
      size: row.size,
      results,
    });
  });

  const aggregateResults: ReportRow["results"] = {};
  backends.forEach((backend) => {
    const ratio = geometricMean(ratiosByBackend[backend]);
    if (ratio === undefined) return;
    maxRatio = Math.max(maxRatio, ratio);
    aggregateResults[backend] = { ratio };
  });
  reportRows.push({
    groupId: "",
    label: "几何平均",
    size: "",
    aggregate: true,
    results: aggregateResults,
  });

  return { rows: reportRows, maxRatio };
}

function escapeXml(value: string): string {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&apos;");
}

function interpolateColor(from: string, to: string, amount: number): string {
  const channels = [0, 2, 4].map((offset) => {
    const start = Number.parseInt(from.slice(offset, offset + 2), 16);
    const end = Number.parseInt(to.slice(offset, offset + 2), 16);
    return Math.round(start + (end - start) * amount)
      .toString(16)
      .padStart(2, "0");
  });
  return `#${channels.join("")}`;
}

function colorForRatio(ratio: number | undefined, maxRatio: number): string {
  if (ratio === undefined) return "#eceff1";
  const linearAmount = Math.max(0, Math.min(1, Math.log(ratio) / Math.log(Math.max(1.01, maxRatio))));
  const amount = Math.sqrt(linearAmount);
  if (amount <= 0.5) return interpolateColor("63be7b", "ffeb84", amount * 2);
  return interpolateColor("ffeb84", "f8696b", (amount - 0.5) * 2);
}

function formatEstimate(estimate: ReportCell): { median: string; margin: string } {
  let scale = 1;
  let unit = "ns";
  let digits = 1;
  if (estimate.median >= 1_000_000) {
    scale = 1_000_000;
    unit = "ms";
    digits = 2;
  } else if (estimate.median >= 1_000) {
    scale = 1_000;
    unit = "μs";
    digits = 2;
  }

  const factor = 10 ** digits;
  const median = Number((estimate.median / scale).toFixed(digits));
  const displayMedianNs = median * scale;
  const margin = Math.max(
    Math.abs(displayMedianNs - estimate.low),
    Math.abs(estimate.high - displayMedianNs),
  ) / scale;
  const roundedMargin = Math.ceil(margin * factor - 1e-9) / factor;
  return {
    median: `${median.toFixed(digits)} ${unit}`,
    margin: `± ${roundedMargin.toFixed(digits)} ${unit}`,
  };
}

function wrapSvgText(value: string, maxCharacters: number): string[] {
  const lines: string[] = [];
  let line = "";
  for (const character of value) {
    if (line.length >= maxCharacters) {
      lines.push(line);
      line = "";
    }
    line += character;
  }
  if (line) lines.push(line);
  return lines;
}

function makeSvgText(
  x: number,
  y: number,
  value: string,
  options: { size?: number; weight?: number; color?: string; anchor?: "start" | "middle"; family?: string } = {},
): string {
  const {
    size = 13,
    weight = 400,
    color = "#20252b",
    anchor = "start",
    family = `-apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif`,
  } = options;
  return `<text x="${x}" y="${y}" text-anchor="${anchor}" dominant-baseline="central" fill="${color}" font-family="${escapeXml(family)}" font-size="${size}" font-weight="${weight}">${escapeXml(value)}</text>`;
}

function makeSvg(rows: ReportRow[], maxRatio: number): string {
  const columnWidths = [245, 275, 340, 340, 340];
  const columnStarts = columnWidths.map((_, index) =>
    columnWidths.slice(0, index).reduce((sum, width) => sum + width, 0),
  );
  const width = columnWidths.reduce((sum, value) => sum + value, 0);
  const headerHeight = 40;
  const rowHeight = 53;
  const height = headerHeight + rows.length * rowHeight;
  const output = [
    `<?xml version="1.0" encoding="UTF-8"?>`,
    `<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}" viewBox="0 0 ${width} ${height}" role="img" aria-labelledby="title description">`,
    `<title id="title">recs / Bevy ECS 基准结果</title>`,
    `<desc id="description">不同 ECS 实现的基准耗时热力表</desc>`,
    `<rect width="100%" height="100%" fill="#fff"/>`,
  ];
  const rect = (x: number, y: number, rectWidth: number, rectHeight: number, fill: string) => {
    output.push(`<rect x="${x}" y="${y}" width="${rectWidth}" height="${rectHeight}" fill="${fill}" stroke="#fff" stroke-width="1"/>`);
  };

  for (let column = 0; column < columnWidths.length; column += 1) {
    rect(columnStarts[column], 0, columnWidths[column], headerHeight, "#edf0f2");
  }

  const bodyRows = rows.map((row) => ({
    ...row,
    displayLabel: row.aggregate
      ? row.label
      : [row.label, formatEntityCount(row.size)].filter(Boolean).join(" · "),
  }));
  for (let index = 0; index < bodyRows.length; index += 1) {
    const row = bodyRows[index];
    const y = headerHeight + index * rowHeight;
    const isGroupStart = !row.aggregate && (index === 0 || bodyRows[index - 1].groupId !== row.groupId);
    if (isGroupStart) {
      let rowSpan = 1;
      while (
        index + rowSpan < bodyRows.length &&
        !bodyRows[index + rowSpan].aggregate &&
        bodyRows[index + rowSpan].groupId === row.groupId
      ) {
        rowSpan += 1;
      }
      rect(0, y, columnWidths[0], rowHeight * rowSpan, "#eaf0f5");
    } else if (row.aggregate) {
      rect(0, y, columnWidths[0], rowHeight, "#eaf0f5");
    }

    rect(columnStarts[1], y, columnWidths[1], rowHeight, row.aggregate ? "#e9edf0" : "#f7f8f9");
    for (let backendIndex = 0; backendIndex < backends.length; backendIndex += 1) {
      const backend = backends[backendIndex];
      const result = row.results[backend];
      rect(
        columnStarts[backendIndex + 2],
        y,
        columnWidths[backendIndex + 2],
        rowHeight,
        colorForRatio(result?.ratio, maxRatio),
      );
    }
  }

  output.push(`<line x1="0" y1="${headerHeight - 0.5}" x2="${width}" y2="${headerHeight - 0.5}" stroke="#d5dbe0" stroke-width="1"/>`);
  for (let index = 0; index < bodyRows.length; index += 1) {
    const row = bodyRows[index];
    if (!row.aggregate && index > 0 && bodyRows[index - 1].groupId === row.groupId) continue;
    const color = row.aggregate ? "#737c86" : "#cbd2d8";
    const y = headerHeight + index * rowHeight + 0.5;
    output.push(`<line x1="0" y1="${y}" x2="${width}" y2="${y}" stroke="${color}" stroke-width="2"/>`);
  }

  const headers = ["Group / API", "参数（实体数）", ...backends.map((backend) => backendLabels[backend])];
  headers.forEach((label, index) => {
    const x = index < 2 ? columnStarts[index] + 12 : columnStarts[index] + columnWidths[index] / 2;
    output.push(makeSvgText(x, headerHeight / 2, label, { weight: 650, anchor: index < 2 ? "start" : "middle" }));
  });

  for (let index = 0; index < bodyRows.length; index += 1) {
    const row = bodyRows[index];
    const y = headerHeight + index * rowHeight;
    const isGroupStart = !row.aggregate && (index === 0 || bodyRows[index - 1].groupId !== row.groupId);
    if (isGroupStart) {
      let rowSpan = 1;
      while (
        index + rowSpan < bodyRows.length &&
        !bodyRows[index + rowSpan].aggregate &&
        bodyRows[index + rowSpan].groupId === row.groupId
      ) {
        rowSpan += 1;
      }
      const groupHeight = rowHeight * rowSpan;
      const label = groupLabels[row.groupId] ?? row.groupId;
      const key = row.groupId;
      const descriptionLines = wrapSvgText(groupDescriptions[row.groupId] ?? "", 19);
      const contentHeight = 16 + 5 + 15 + 7 + descriptionLines.length * 15;
      let contentY = y + (groupHeight - contentHeight) / 2;
      output.push(makeSvgText(12, contentY + 8, label, { weight: 700 }));
      contentY += 21;
      output.push(makeSvgText(12, contentY + 7.5, key, {
        size: 11,
        color: "#55616d",
        family: `ui-monospace, SFMono-Regular, Menlo, Consolas, monospace`,
      }));
      contentY += 22;
      descriptionLines.forEach((line, lineIndex) => {
        output.push(makeSvgText(12, contentY + 7.5 + lineIndex * 15, line, {
          size: 11,
          color: "#46515c",
          weight: 400,
        }));
      });
    } else if (row.aggregate) {
      output.push(makeSvgText(12, y + rowHeight / 2, "汇总", { weight: 700 }));
    }

    output.push(makeSvgText(columnStarts[1] + 12, y + rowHeight / 2, row.displayLabel, {
      weight: row.aggregate ? 700 : 550,
    }));
    for (let backendIndex = 0; backendIndex < backends.length; backendIndex += 1) {
      const backend = backends[backendIndex];
      const x = columnStarts[backendIndex + 2] + columnWidths[backendIndex + 2] / 2;
      const result = row.results[backend];
      if (!result) {
        output.push(makeSvgText(x, y + rowHeight / 2, "—", { color: "#6b737c", anchor: "middle" }));
      } else if (row.aggregate) {
        output.push(makeSvgText(x, y + rowHeight / 2, `${result.ratio.toFixed(2)}×`, {
          size: 15,
          weight: 700,
          anchor: "middle",
        }));
      } else {
        const estimate = formatEstimate(result as ReportCell);
        output.push(`<text x="${x}" y="${y + 18}" text-anchor="middle" dominant-baseline="central" fill="#20252b" font-family="-apple-system, BlinkMacSystemFont, &quot;Segoe UI&quot;, sans-serif" font-size="14" font-weight="650"><tspan>${escapeXml(estimate.median)}</tspan><tspan fill="#4c5560" font-size="11" font-weight="450"> ${escapeXml(estimate.margin)}</tspan></text>`);
        output.push(makeSvgText(x, y + 37, `(${result.ratio.toFixed(2)}×)`, {
          size: 11,
          weight: 650,
          color: "#303740",
          anchor: "middle",
        }));
      }
    }
  }

  output.push("</svg>");
  return `${output.join("\n")}\n`;
}

function makeDataScript(rows: ResultRow[]): string {
  const report = toReportRows(rows);
  const data = {
    backendIds: backends,
    backends: backends.map((backend) => backendLabels[backend]),
    groupLabels,
    groupDescriptions,
    rows: report.rows.map((row) => ({
      groupId: row.groupId,
      label: row.aggregate
        ? row.label
        : [row.label, formatEntityCount(row.size)].filter(Boolean).join(" · "),
      aggregate: row.aggregate,
      results: row.results,
    })),
    maxRatio: Math.max(2, report.maxRatio),
  };
  const json = JSON.stringify(data).replaceAll("<", "\\u003c");
  return `window.ECS_BENCHMARK = ${json};\n`;
}

if (!existsSync(criterionRoot)) {
  console.error("No Criterion results found under target/criterion. Run `mise run bench` first.");
  process.exitCode = 1;
} else {
  const rows = new Map<string, ResultRow>();
  let ignoredOldGroups = 0;

  for (const benchmarkPath of walk(criterionRoot)) {
    const benchmark = JSON.parse(readFileSync(benchmarkPath, "utf8")) as Benchmark;
    if (!allowedGroups.has(benchmark.group_id)) {
      ignoredOldGroups++;
      continue;
    }

    const backend = backendFor(benchmark.function_id);
    if (!backend) continue;

    const estimatesPath = join(dirname(benchmarkPath), "estimates.json");
    const estimate = JSON.parse(readFileSync(resolve(estimatesPath), "utf8")) as Estimate;
    const { key, workload } = benchmarkKey(benchmark);
    const row = rows.get(key) ?? {
      key,
      groupId: benchmark.group_id,
      workload,
      size: benchmark.value_str,
      results: {},
    };
    row.results[backend] = estimate;
    rows.set(key, row);
  }

  const sortedRows = [...rows.values()].sort(naturalCompare);
  if (sortedRows.length === 0) {
    console.error("No matching ECS benchmark results found. Run `mise run bench` first.");
    process.exitCode = 1;
  } else {
    const report = toReportRows(sortedRows);
    mkdirSync(criterionRoot, { recursive: true });
    copyFileSync(join(repoRoot, "benches", "report", "index.html"), reportPath);
    copyFileSync(join(repoRoot, "benches", "report", "heatmap.css"), join(criterionRoot, "ecs-heatmap.css"));
    copyFileSync(join(repoRoot, "benches", "report", "heatmap.js"), join(criterionRoot, "ecs-heatmap.js"));
    writeFileSync(join(criterionRoot, "ecs-heatmap-data.js"), makeDataScript(sortedRows));
    writeFileSync(reportSvgPath, makeSvg(report.rows, Math.max(2, report.maxRatio)));
    console.log(`Benchmark heatmap report written to ${reportPath}`);
    console.log(`Benchmark heatmap SVG written to ${reportSvgPath}`);
    console.log(`Included ${sortedRows.length} workload/size combinations across ${backends.length} implementations.`);
    if (ignoredOldGroups > 0) {
      console.log(`Ignored ${ignoredOldGroups} Criterion entries from older benchmark layouts.`);
    }
  }
}
