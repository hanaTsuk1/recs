# recs

记录一些 ecs 设计上的方案以及对应原因，基于已有的 ecs 实现方向来进行学习，并重新实现

## entity

| 设计                  | 原因                           |
| --------------------- | ------------------------------ |
| Entity 使用整数 ID    | 轻量、便于索引和管理           |
| 允许 ID 复用          | 避免槽位不断增长，重复利用资源 |
| 增加 Generation       | 防止旧 Entity 误认成新 Entity  |
| Entity 不直接持有组件 | 身份与数据存储分离             |



**为什么销毁 Entity 时就增加 Generation，而不是创建新 Entity 时才增加？**

两种时机都能设计出正确方案，但销毁时递增可以让槽位一旦释放，旧句柄就立即失效。



## component

将 component 集中存储而不是让 entity 持有

+ 不同 Entity 拥有不同的 Component
+ Entity 直接包含所有 Component，那么每增加一种 Component，都可能需要修改 Entity 的结构
+ Component 分散在每个 Entity 内部，那么遍历所有 Component 就不够直接

### Sparse Set

**如何在保持 Component 连续存储的同时，快速通过 Entity 找到对应的 Component？**

- Sparse：建立 Entity ID 到 Dense 位置的映射
- Dense：紧凑地存储实际数据。

**为什么删除组件时通常使用 swap_remove，而不是普通删除？**

1. **索引维护成本**：普通删除会导致后续 Dense 元素索引发生变化，因此必须更新对应的 Sparse 映射。
2. **数据移动成本**：普通删除需要移动后续所有 Component，删除位置越靠前，移动的数据越多。

