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



sparse set 解决了单个类型 component 的存储，现在要管理不同类型的 component

+ 需要一种方式实现 component 到 sparse set 的映射，但不能把类型写死成确定的 component

### **Component Registry**

**我们怎么在一个统一的数据结构中管理不同类型的 SparseSet？**

+ std::any::TypeId，可以在运行时标识一个具体的类型
+ 将SparseSet\<T\>作为Trait Object 就可以统一管理



### Component trait

1. **Component 缺少统一的类型约束**：之前使用 `'static` 约束组件类型，没有专门的 `Component` Trait 来表达组件的身份和边界。
2. **Bundle 不支持单个普通 Struct 自动接入**：之前主要通过 Tuple 实现 Bundle，普通 Struct 想作为单个组件使用时，缺少统一的类型约束和自动 Bundle 实现。
3. **选择 Bevy 风格的 Component 设计，而不是 hecs 风格**：
- Bevy 风格：`Component` 需要显式实现（后续通过 `#[derive(Component)]` 简化）。
- hecs 风格：通过 blanket impl，让满足 `Send + Sync + 'static` 的类型自动成为 Component。



## world

现在 entity 和 component 都有了，需要一个统一管理入口

+ **Entity 与 Component 的一致性**：销毁 entity 也要保证对应 component 全部被销毁，entity 的有效性要传达到 component



要用 entity 销毁对应 component，就得遍历storages 这个值为`Box<dyn Any>`的hashmap

**只指定 entity 没有对应的 T， 要怎么通过`Box<dyn Any>`获取`SparseSet<T>` 来进行销毁呢？**

+ 用 trait Storage: any，得到具体的`Box<dyn Storage>`，然后让 sparse set 实现 storage，就可以通过 trait object 来调用SparseSet 的确定函数，而不需要管 T



## query

现在开始查询数据，我们可能面临各种类型，甚至是组合的类型需要一起返回结果

- `&Position`
- `&mut Position`
- `(&Position, &Velocity)`
- `(&mut Position, &Velocity)`

需要一个 trait 来约束类型，将这些限制为 component

通过这个 `QueryData` trait获取 world 中的`ComponentRegistry`找到对应 storage

最后 query 返回迭代器

---

一旦照着这个路线为**不可变**、**可变**及**其组合**依次实现，就会在获取 storage含有**可变**的组合时遇到rust的限制

> 同一时刻，你只能拥有要么一个可变引用，要么任意多个不可变引用

无法在通过world获取storage 时，同时得到多个引用**包含可变引用**的情况

这时候就需要使用裸指针了，通过query内部独占world，然后对存储component的sparse set进行解引用，让遍历entity的过程所产生的结果不发生重叠

由于world独占，component的存储不会发生变化，只需要保证查询的类型读写本身不重叠，最终可以保证操作安全
