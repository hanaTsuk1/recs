const report = window.ECS_BENCHMARK;
const root = document.getElementById("report");

function interpolateColor(from, to, amount) {
  const channels = [0, 2, 4].map((offset) => {
    const start = Number.parseInt(from.slice(offset, offset + 2), 16);
    const end = Number.parseInt(to.slice(offset, offset + 2), 16);
    return Math.round(start + (end - start) * amount)
      .toString(16)
      .padStart(2, "0");
  });
  return `#${channels.join("")}`;
}

function colorForRatio(ratio) {
  if (ratio === undefined) return "#eceff1";
  const maxRatio = Math.max(1.01, report.maxRatio);
  const linearAmount = Math.max(0, Math.min(1, Math.log(ratio) / Math.log(maxRatio)));
  const amount = Math.sqrt(linearAmount);
  if (amount <= 0.5) return interpolateColor("63be7b", "ffeb84", amount * 2);
  return interpolateColor("ffeb84", "f8696b", (amount - 0.5) * 2);
}

function formatEstimate(estimate) {
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

function appendHeader(table) {
  const head = document.createElement("thead");
  const row = document.createElement("tr");
  for (const label of ["Group / API", "参数（实体数）", ...report.backends]) {
    const cell = document.createElement("th");
    cell.scope = "col";
    cell.textContent = label;
    row.append(cell);
  }
  head.append(row);
  table.append(head);
}

function appendGroupCell(rowElement, groupId, rowSpan) {
  const cell = document.createElement("td");
  cell.className = "group-cell";
  cell.rowSpan = rowSpan;

  const label = document.createElement("span");
  label.className = "group-label";
  label.textContent = report.groupLabels[groupId] ?? groupId;

  const key = document.createElement("code");
  key.className = "group-key";
  key.textContent = groupId;

  const description = document.createElement("span");
  description.className = "group-description";
  description.textContent = report.groupDescriptions[groupId] ?? "";
  cell.append(label, key, description);
  rowElement.append(cell);
}

function appendResultCell(rowElement, backend, row) {
  const cell = document.createElement("td");
  const result = row.results[backend];
  if (!result) {
    cell.className = "measurement missing";
    cell.textContent = "—";
    rowElement.append(cell);
    return;
  }

  cell.className = "measurement";
  cell.style.backgroundColor = colorForRatio(result.ratio);
  if (row.aggregate) {
    cell.classList.add("aggregate-value");
    cell.textContent = `${result.ratio.toFixed(2)}×`;
  } else {
    const time = document.createElement("span");
    time.className = "time-value";
    const estimate = formatEstimate(result);
    time.textContent = estimate.median;

    const margin = document.createElement("small");
    margin.className = "time-error";
    margin.textContent = estimate.margin;
    time.append(" ", margin);

    const ratio = document.createElement("span");
    ratio.className = "relative-factor";
    ratio.textContent = `(${result.ratio.toFixed(2)}×)`;
    cell.append(time, ratio);
  }
  rowElement.append(cell);
}

const table = document.createElement("table");
table.className = "benchmark-table";
table.setAttribute("aria-label", "ECS benchmark 结果对比");
appendHeader(table);

const body = document.createElement("tbody");
for (let index = 0; index < report.rows.length; index += 1) {
  const row = report.rows[index];
  const tableRow = document.createElement("tr");
  if (row.aggregate) tableRow.className = "geometric-mean";

  if (row.aggregate) {
    const summary = document.createElement("td");
    summary.className = "group-cell summary-group";
    summary.textContent = "汇总";
    tableRow.append(summary);
  } else if (index === 0 || report.rows[index - 1].groupId !== row.groupId) {
    let rowSpan = 1;
    while (
      index + rowSpan < report.rows.length &&
      !report.rows[index + rowSpan].aggregate &&
      report.rows[index + rowSpan].groupId === row.groupId
    ) {
      rowSpan += 1;
    }
    appendGroupCell(tableRow, row.groupId, rowSpan);
    tableRow.classList.add("group-start");
  }

  const workload = document.createElement("th");
  workload.scope = "row";
  workload.textContent = row.label;
  tableRow.append(workload);

  for (const backend of report.backendIds) {
    appendResultCell(tableRow, backend, row);
  }
  body.append(tableRow);
}

table.append(body);
root.append(table);
