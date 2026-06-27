# Oxigraph .NET Bindings — Design Spec

> 状态: **已评审**
> 日期: 2026-06-27
> 分支: `dotnet`
> 从头脑风暴产出: 11 个关键决策确认

---

## 1. 概述与目标

为 Oxigraph 提供原生 .NET 10+ 绑定，对标 Python 绑定 (`pyoxigraph`) 的能力完整度。用户通过 NuGet 安装 `Oxigraph` 包即可使用完整 Store / SPARQL / I/O 功能。

### 核心目标

1. 原生 .NET 10+ 绑定，完整暴露 Oxigraph 的 Store、SPARQL、I/O 能力
2. 支持 RocksDB 文件持久化和内存模式
3. 通过可选扩展包与 dotNetRDF 互操作（分期交付）
4. 跨平台支持（Windows / Linux / macOS）

---

## 2. 关键决策

以下决策已在头脑风暴阶段确认，作为设计的不可变约束：

| # | 决策点 | 选择 | 理由 |
|---|---|---|---|
| 1 | dotNetRDF 扩展 | 分期交付，核心包先行 | 降低初期复杂度，核心包扎实后再做适配 |
| 2 | C# API 命名 | .NET 惯用风格（PascalCase） | 符合 .NET 生态预期 |
| 3 | FFI 数据传递 | JSON 序列化 | FFI 签名简洁，无跨 FFI 对齐风险 |
| 4 | 错误处理 | 统一 JSON `{"ok":...}` / `{"error":...}` | 和 JSON 数据流一致，FFI 签名统一 |
| 5 | 异步/线程 | 纯同步 API | .NET 无 GIL，用户按需 `Task.Run` |
| 6 | NuGet 命名 | `Oxigraph` + `Oxigraph.Extensions.DotNetRDF` | 核心包零外部依赖 |
| 7 | 原生二进制分发 | 嵌入 NuGet (`runtimes/{rid}/native/`) | 对标 Python wheel，零配置 |
| 8 | 线程安全 | 不保证，文档标注 | RocksDB 自身需要调用者协调并发 |
| 9 | SPARQL 结果 | 分页游标 + `IEnumerable<T>` | 对标 Python 惰性迭代，大结果集可控 |
| 10 | 构建系统 | csbindgen + dotnet pack | 自动生成 [LibraryImport] 绑定 |
| 11 | 测试 | xUnit，复用 Python 测试夹具 | .NET 生态标准 |

---

## 3. 架构

```
┌──────────────────────────────────────────────────────────────┐
│  C# 用户代码                                                  │
│  var store = new Store("/path/to/db");                       │
│  store.Add(new Quad(s, p, o, g));                           │
│  var results = store.Query("SELECT ...");                    │
└──────────────────────────┬───────────────────────────────────┘
                           │
┌──────────────────────────▼───────────────────────────────────┐
│  C# 公共 API 层 (Oxigraph)                                    │
│  Model/*.cs     Store.cs     IO.cs     Sparql.cs              │
│  - PascalCase 命名 (Add, Match, BulkLoad)                     │
│  - 纯同步 API                                                │
│  - 无锁，线程安全由调用者负责                                   │
└──────────────────────────┬───────────────────────────────────┘
                           │ 内部调用
┌──────────────────────────▼───────────────────────────────────┐
│  FFI 封装层 (Oxigraph/Interop/)                               │
│  NativeMethods.g.cs (csbindgen 生成)                          │
│  FFIHelper.cs (统一 JSON 调用/错误处理)                        │
│  SafeHandles.cs (StoreHandle, CursorHandle)                   │
│  - [LibraryImport] 声明, .NET 10+ 源生成器                     │
│  - 调用 → JSON 字符串返回 → JsonSerializer.Deserialize        │
│  - {"error":...} → OxigraphException 子类                     │
└──────────────────────────┬───────────────────────────────────┘
                           │ P/Invoke
┌──────────────────────────▼───────────────────────────────────┐
│  Rust cdylib (oxigraph-dotnet)                               │
│  ffi.rs / model_ffi.rs / store_ffi.rs / io_ffi.rs /           │
│  sparql_ffi.rs / error.rs                                    │
│  - extern "C" fn 接受/返回 *const c_char / *mut c_char (JSON) │
│  - 统一返回: {"ok": result} 或 {"error": {"kind":..., ...}}   │
│  - 内存: StoreHandle, CursorHandle (Box::into_raw, C# SafeHandle) │
│  - 数据: JSON 按值复制，无需手动管理                            │
└──────────────────────────┬───────────────────────────────────┘
                           │
┌──────────────────────────▼───────────────────────────────────┐
│  Oxigraph Core (lib/oxigraph)                                │
│  Store / RdfParser / SparqlEvaluator / ...                   │
└──────────────────────────────────────────────────────────────┘
```

---

## 4. FFI 设计

### 4.1 核心约定

所有 Rust `extern "C"` 函数遵循统一签名：

```
输入:  必需的 *const c_char (JSON 参数, 可为 null)
       必需的 StoreHandle / CursorHandle (操作对象)
输出:  *mut c_char (JSON 字符串, 调用者通过 oxigraph_free_string 释放)
       例外: store_destroy, cursor_destroy, free_string 等析构函数返回 void

返回值始终为 JSON:
  成功 → {"ok": <result>}
  失败 → {"error": {"kind": "<error_type>", "message": "<description>", ...}}

Stream 数据: Load(Stream)/Dump(Stream) 在 C# 侧将 Stream 读为 byte[]，
  通过 oxigraph_store_load_bytes(handle, data_ptr, data_len, options_json) 传递。
  不经过 JSON 字符串编码，避免 base64 膨胀。

C# 侧统一入口 (FFIHelper.cs):
  T Call<T>(Func<IntPtr> ffiCall) → 反序列化 "ok" 字段
  void CallVoid(Func<IntPtr> ffiCall) → 仅检查是否有 "error"
  MapError(json) → 根据 kind 抛出对应 OxigraphException 子类
```

### 4.2 句柄类型

```rust
pub type StoreHandle = *mut std::cell::UnsafeCell<Store>;
pub type CursorHandle = *mut std::cell::UnsafeCell<CursorState>;
```

C# 侧:
```csharp
internal sealed class StoreSafeHandle : SafeHandleZeroOrMinusOneIsInvalid { ... }
internal sealed class CursorSafeHandle : SafeHandleZeroOrMinusOneIsInvalid { ... }
```

### 4.3 关键 FFI 函数签名

```rust
// Store 生命周期
extern "C" fn oxigraph_store_new() -> *mut c_char;
extern "C" fn oxigraph_store_open(path: *const c_char) -> *mut c_char;
extern "C" fn oxigraph_store_open_read_only(path: *const c_char) -> *mut c_char;
extern "C" fn oxigraph_store_destroy(handle: StoreHandle);

// Store CRUD
extern "C" fn oxigraph_store_add(handle: StoreHandle, quad_json: *const c_char) -> *mut c_char;
extern "C" fn oxigraph_store_remove(handle: StoreHandle, quad_json: *const c_char) -> *mut c_char;
extern "C" fn oxigraph_store_contains(handle: StoreHandle, quad_json: *const c_char) -> *mut c_char;
extern "C" fn oxigraph_store_match(handle: StoreHandle, pattern_json: *const c_char) -> *mut c_char;
extern "C" fn oxigraph_store_count(handle: StoreHandle) -> *mut c_char;

// SPARQL (返回游标)
extern "C" fn oxigraph_store_query(handle: StoreHandle, query_json: *const c_char) -> *mut c_char;
extern "C" fn oxigraph_store_update(handle: StoreHandle, update_json: *const c_char) -> *mut c_char;

// 游标 (分页)
extern "C" fn oxigraph_cursor_next(cursor: CursorHandle) -> *mut c_char;
extern "C" fn oxigraph_cursor_destroy(cursor: CursorHandle);

// I/O
extern "C" fn oxigraph_parse(input_json: *const c_char) -> *mut c_char;
extern "C" fn oxigraph_serialize(quads_json: *const c_char, options_json: *const c_char) -> *mut c_char;
extern "C" fn oxigraph_store_load(handle: StoreHandle, load_json: *const c_char) -> *mut c_char;
extern "C" fn oxigraph_store_load_bytes(handle: StoreHandle, data: *const u8, data_len: usize, options_json: *const c_char) -> *mut c_char;
extern "C" fn oxigraph_store_dump(handle: StoreHandle, dump_json: *const c_char) -> *mut c_char;
extern "C" fn oxigraph_store_dump_bytes(handle: StoreHandle, dump_json: *const c_char) -> *mut c_char;  // 返回 {"ok": {"data_base64":"..."}} 或直接返回二进制

// 内存管理
extern "C" fn oxigraph_free_string(ptr: *mut c_char);
extern "C" fn oxigraph_free_byte_array(ptr: *mut u8, len: usize);
```

### 4.4 SPARQL 游标协议

```
首次调用:
  store_query(handle, query_json) →
  {"ok": {"result_type":"solutions","variables":["s","p","o"],
          "cursor":"abc123","batch":[{...},...],"has_more":true}}

翻页:
  cursor_next(cursor) →
  {"ok": {"batch":[{...},...],"has_more":true}}
  ...
  {"ok": {"batch":[{...}],"has_more":false}}  (最后一批)

C# 侧:
  class QuerySolutions : IEnumerable<QuerySolution> {
      // 内部在 foreach 中惰性调用 cursor_next
  }
```

---

## 5. C# 公共 API

### 5.1 数据模型

```csharp
namespace Oxigraph;

public sealed record NamedNode(string Value);
public sealed record BlankNode(string Value);
public sealed record Literal(string Value, string? Language = null, NamedNode? Datatype = null);
public readonly record struct DefaultGraph();

public sealed record Triple(NamedOrBlankNode Subject, NamedNode Predicate, Term Object);
public sealed record Quad(NamedOrBlankNode Subject, NamedNode Predicate, Term Object, GraphName Graph);
public sealed record Variable(string Value);

// 联合类型 (abstract record 封闭继承)
public abstract record NamedOrBlankNode();
public abstract record Term();
public abstract record GraphName();
```

### 5.2 Store

```csharp
public sealed class Store : IDisposable
{
    public Store(string? path = null);          // null = 内存模式
    public static Store OpenReadOnly(string path);

    // CRUD
    public void Add(Quad quad);
    public void Remove(Quad quad);
    public bool Contains(Quad quad);
    public ulong Count { get; }
    public bool IsEmpty { get; }

    // 模式匹配
    public IEnumerable<Quad> Match(
        NamedOrBlankNode? subject = null,
        NamedNode? predicate = null,
        Term? @object = null,
        GraphName? graph = null);

    // 批量
    public void Extend(IEnumerable<Quad> quads);
    public void BulkExtend(IEnumerable<Quad> quads);

    // SPARQL
    public QueryResults Query(string sparql, QueryOptions? options = null);
    public void Update(string sparql, UpdateOptions? options = null);

    // I/O
    public void Load(string input, RdfFormat format, LoadOptions? options = null);
    public void Load(Stream stream, RdfFormat format, LoadOptions? options = null);
    public void BulkLoad(string input, RdfFormat format, LoadOptions? options = null);
    public string Dump(RdfFormat format, DumpOptions? options = null);
    public void Dump(Stream output, RdfFormat format, DumpOptions? options = null);

    // 管理
    public void Flush();
    public void Optimize();
    public void Backup(string targetDirectory);
    public void Clear();

    // 命名图
    public void AddGraph(NamedOrBlankNode graphName);
    public void RemoveGraph(NamedOrBlankNode graphName);
    public void ClearGraph(GraphName graphName);
    public bool ContainsNamedGraph(NamedOrBlankNode graphName);
    public IReadOnlyCollection<NamedOrBlankNode> NamedGraphs { get; }

    public void Dispose();
}
```

### 5.3 SPARQL 结果

```csharp
public abstract class QueryResults { }

public sealed class QueryBoolean : QueryResults { public bool Value { get; } }

public sealed class QuerySolutions : QueryResults, IEnumerable<QuerySolution>
{
    public IReadOnlyList<Variable> Variables { get; }
    public IEnumerator<QuerySolution> GetEnumerator();  // 惰性游标分页
}

public sealed class QueryTriples : QueryResults, IEnumerable<Triple>
{
    public IEnumerator<Triple> GetEnumerator();         // 惰性游标分页
}

public sealed class QuerySolution
{
    public Term? this[string variable] { get; }
    public bool TryGetValue(string variable, out Term? value);
}
```

### 5.4 选项与格式

```csharp
public enum RdfFormat { NTriples, NQuads, Turtle, TriG, N3, JsonLd, RdfXml }

public sealed record QueryOptions(
    string? BaseIri = null,
    Dictionary<string, string>? Prefixes = null,
    bool UseDefaultGraphAsUnion = false,
    IReadOnlyList<GraphName>? DefaultGraphs = null,
    IReadOnlyList<GraphName>? NamedGraphs = null);

public sealed record LoadOptions(
    string? BaseIri = null,
    GraphName? ToGraph = null,
    bool Lenient = false);

public sealed record UpdateOptions(
    string? BaseIri = null,
    Dictionary<string, string>? Prefixes = null);

public sealed record DumpOptions(
    GraphName? FromGraph = null,
    string? BaseIri = null,
    Dictionary<string, string>? Prefixes = null);
```

### 5.5 异常体系

```csharp
public class OxigraphException : Exception { ... }
public class StoreException : OxigraphException { ... }
public class ParseException : OxigraphException { public string? FilePath { get; } public int? Line { get; } }
public class SparqlSyntaxException : OxigraphException { ... }
public class SparqlEvaluationException : OxigraphException { ... }
```

---

## 6. 项目结构

```
dotnet/
├── Oxigraph.sln
├── Directory.Build.props              # net10.0, Nullable=enable, ImplicitUsings
├── build_package.py                   # cargo build + csbindgen + dotnet pack
├── src/
│   ├── oxigraph-dotnet/               # Rust cdylib crate
│   │   ├── Cargo.toml                 # [lib] crate-type=["cdylib"]
│   │   └── src/
│   │       ├── lib.rs                 # 模块声明
│   │       ├── ffi.rs                 # extern "C" 导出 (csbindgen 扫描目标)
│   │       ├── model_ffi.rs           # Quad/Term JSON 转换
│   │       ├── store_ffi.rs           # Store FFI 函数
│   │       ├── io_ffi.rs              # parse/serialize/load/dump
│   │       ├── sparql_ffi.rs          # Query/Update + 游标
│   │       └── error.rs               # 错误 kind 枚举 + JSON 序列化
│   └── Oxigraph/                      # C# 类库
│       ├── Oxigraph.csproj
│       ├── Interop/
│       │   ├── NativeMethods.g.cs     # csbindgen 自动生成
│       │   ├── FFIHelper.cs           # Call<T>, CallVoid, MapError
│       │   └── SafeHandles.cs         # StoreSafeHandle, CursorSafeHandle
│       ├── Model/
│       │   ├── NamedNode.cs
│       │   ├── BlankNode.cs
│       │   ├── Literal.cs
│       │   ├── DefaultGraph.cs
│       │   ├── Triple.cs
│       │   ├── Quad.cs
│       │   ├── Variable.cs
│       │   └── Term.cs                # Term, GraphName, NamedOrBlankNode 基类
│       ├── Store.cs
│       ├── IO.cs                      # static Parse, Serialize
│       ├── Sparql.cs                  # QueryResults 及子类
│       ├── RdfFormat.cs
│       └── Exceptions.cs              # OxigraphException 体系
└── tests/
    └── Oxigraph.Tests/
        ├── Oxigraph.Tests.csproj
        ├── Interop/
        │   └── FFIMarshallingTests.cs
        ├── StoreTests.cs
        ├── IOTests.cs
        └── SparqlTests.cs
```

### 根 Cargo.toml 修改

```toml
[workspace]
members = [
    # ... existing 18 members ...
    "dotnet/src/oxigraph-dotnet",
]
```

---

## 7. 构建系统

### build_package.py 流程

```
1. cargo build --release                              # cdylib → target/release/
2. csbindgen --input src/oxigraph-dotnet/src/ffi.rs   # Rust → C# [LibraryImport]
             --output src/Oxigraph/Interop/NativeMethods.g.cs
3. dotnet build src/Oxigraph/                          # 编译 C# 类库
4. dotnet test tests/Oxigraph.Tests/                   # 运行测试
5. dotnet pack src/Oxigraph/                           # 生成 .nupkg
```

### NuGet 包结构

```
Oxigraph.1.0.0.nupkg
├── lib/net10.0/
│   ├── Oxigraph.dll
│   └── Oxigraph.xml
├── runtimes/
│   ├── win-x64/native/oxigraph_native.dll
│   ├── win-arm64/native/oxigraph_native.dll
│   ├── linux-x64/native/liboxigraph_native.so
│   ├── linux-arm64/native/liboxigraph_native.so
│   ├── osx-x64/native/liboxigraph_native.dylib
│   └── osx-arm64/native/liboxigraph_native.dylib
└── build/Oxigraph.targets          # MSBuild 自动加载原生库
```

### 依赖

| 层 | 依赖 | 说明 |
|---|---|---|
| Rust | `oxigraph` (core), `serde_json` | 无额外 FFI 框架 |
| C# | `System.Text.Json` (内置) | 零第三方 NuGet 依赖 |
| 构建 | `csbindgen` (cargo install), `dotnet` SDK 10.0+ | 开发机工具链 |

---

## 8. 测试策略

### 测试框架: xUnit

### 测试分层

| 类别 | 路径 | 验证点 |
|---|---|---|
| **FFI 往返** | `Interop/FFIMarshallingTests.cs` | Quad/QueryResults JSON ↔ Rust 序列化对称性 |
| **内存管理** | `Interop/FFIMarshallingTests.cs` | 1000× Store 创建/Dispose, GC 后无泄漏 |
| **Store CRUD** | `StoreTests.cs` | Add/Contains/Remove/Count/Match 基本语义 |
| **文件持久化** | `StoreTests.cs` | Store(path)→add→dispose→new Store(path)→contains |
| **命名图** | `StoreTests.cs` | AddGraph/RemoveGraph/ClearGraph/NamedGraphs |
| **SPARQL** | `SparqlTests.cs` | SELECT/CONSTRUCT/ASK/UPDATE 正确性 |
| **SPARQL 分页** | `SparqlTests.cs` | 大结果集游标透明翻页 |
| **I/O 格式** | `IOTests.cs` | 7 种格式 Parse ↔ Serialize 往返 |
| **错误** | `*Tests.cs` | 无效 IRI/破损 Turtle/不存在目录 → 正确异常子类 |

### 测试夹具

复用 Python 绑定已有的测试数据片段（小 Turtle/NT/RDF-XML 片段），避免重新造数据。

### Phase 1 聚焦

Phase 1 (PoC) 仅测试 FFI 往返 + Store CRUD 内存模式 + 基本 Parse。其余随 Phase 2-4 添加。

---

## 9. 实施阶段

| 阶段 | 内容 | 时长 |
|---|---|---|
| **Phase 1: PoC** | Rust cdylib 搭建, store_new/add/contains/destroy FFI, C# [LibraryImport] 调用, Quad JSON 往返, 内存管理测试 | 2-3 周 |
| **Phase 2: Model** | 完整 RDF 数据模型 FFI + C# 封装 + 单元测试 | 2 周 |
| **Phase 3: Store** | 完整 Store API + SPARQL Query/Update + 游标分页 + 文件持久化 + 命名图管理 | 3 周 |
| **Phase 4: I/O** | Parse/Serialize/Load/Dump 全部 7 种格式 | 1 周 |
| **Phase 5: dotNetRDF** | 适配器扩展包 (ITripleStore, INode, ISparqlQueryProcessor) | 1 周 |
| **Phase 6: CI/CD** | csbindgen 集成构建, 跨平台 CI, NuGet 打包发布 | 1 周 |

**总计: 10-11 周**

---

## 10. 风险

| 风险 | 缓解 |
|---|---|
| SPARQL 游标生命周期复杂 (内存泄漏 / use-after-free) | 参照 Python QuadIter 模式, CursorSafeHandle + IEnumerable 保证 Dispose |
| 跨平台 .so/.dll/.dylib 加载失败 | NativeLibrary.SetDllImportResolver + NuGet runtimes/rid/native 标准结构 |
| RocksDB Windows 编译 | 已有 oxrocksdb-sys 解决方案 (Python MSVC CI 已验证) |
| csbindgen 不覆盖复杂场景 | 覆盖 80% 场景, 剩余在 NativeMethods.custom.cs 手写 |
| Quad JSON 序列化性能 | 小数据按值传递, 批量操作用流式 JSON 数组而非逐条调用 |
