using System.Collections;
using System.Text.Json;

namespace Oxigraph;

/// <summary>
/// An in-memory RDF dataset. Supports quad CRUD and pattern matching.
/// For file persistence and SPARQL, use <see cref="Store"/> instead.
///
/// Warning: This interning structure does not garbage-collect removed terms —
/// if you insert and remove many different terms, memory grows without reduction.
/// </summary>
public sealed class Dataset : IEnumerable<Quad>, IDisposable
{
    private readonly Store _store;

    /// <summary>Create an empty dataset.</summary>
    public Dataset()
    {
        _store = new Store();
    }

    /// <summary>Create a dataset initialized with the given quads.</summary>
    public Dataset(IEnumerable<Quad> quads) : this()
    {
        foreach (var q in quads)
            _store.Add(q);
    }

    // ─── CRUD ─────────────────────────────

    /// <summary>Insert a quad into the dataset.</summary>
    public void Add(Quad quad) => _store.Add(quad);

    /// <summary>Remove a quad from the dataset.</summary>
    public void Remove(Quad quad) => _store.Remove(quad);

    /// <summary>Check if the dataset contains a quad.</summary>
    public bool Contains(Quad quad) => _store.Contains(quad);

    /// <summary>Number of quads in the dataset.</summary>
    public int Count => (int)_store.Count;

    /// <summary>Whether the dataset is empty.</summary>
    public bool IsEmpty => _store.IsEmpty;

    // ─── Pattern matching ─────────────────

    /// <summary>Find all quads matching the given pattern.</summary>
    public IReadOnlyList<Quad> Match(
        INamedOrBlankNode? subject = null,
        NamedNode? predicate = null,
        ITerm? @object = null,
        IGraphName? graph = null)
        => _store.Match(subject, predicate, @object, graph);

    /// <summary>Find quads with the given subject.</summary>
    public IReadOnlyList<Quad> QuadsForSubject(INamedOrBlankNode subject)
        => Match(subject: subject);

    /// <summary>Find quads with the given predicate.</summary>
    public IReadOnlyList<Quad> QuadsForPredicate(NamedNode predicate)
        => Match(predicate: predicate);

    /// <summary>Find quads with the given object.</summary>
    public IReadOnlyList<Quad> QuadsForObject(ITerm @object)
        => Match(@object: @object);

    // ─── Manipulation ─────────────────────

    /// <summary>Remove all quads from the dataset.</summary>
    public void Clear() => _store.Clear();

    /// <summary>Add all quads from another dataset or collection.</summary>
    public void Extend(IEnumerable<Quad> quads) => _store.Extend(quads);

    // ─── Serialization ────────────────────

    /// <summary>Dump the dataset as N-Quads text.</summary>
    public string Dump(RdfFormat format = RdfFormat.NQuads, DumpOptions? options = null)
        => _store.Dump(format, options);

    /// <summary>Load quads from RDF text.</summary>
    public void Load(string data, RdfFormat format, LoadOptions? options = null)
        => _store.Load(data, format, options);

    // ─── IEnumerable ──────────────────────

    public IEnumerator<Quad> GetEnumerator()
    {
        foreach (var q in _store.Match())
            yield return q;
    }

    IEnumerator IEnumerable.GetEnumerator() => GetEnumerator();

    public void Dispose() => _store.Dispose();

    /// <summary>N-Quads serialization of the dataset.</summary>
    public override string ToString() => Dump(RdfFormat.NQuads);
}