const { MemoryStore } = require('../pkg/oxigraph.js');
const assert = require('assert');
const dataFactory = require('@rdfjs/data-model');

const ex = dataFactory.namedNode('http://example.com');

describe('MemoryStore', function() {
  describe('#add()', function() {
    it('an added quad should be in the store', function() {
      const store = new MemoryStore();
      store.add(dataFactory.triple(ex, ex, ex));
      assert(store.has(dataFactory.triple(ex, ex, ex)));
    });
  });

  describe('#delete()', function() {
    it('an removed quad should not be in the store anymore', function() {
      const store = new MemoryStore([dataFactory.triple(ex, ex, ex)]);
      assert(store.has(dataFactory.triple(ex, ex, ex)));
      store.delete(dataFactory.triple(ex, ex, ex))
      assert(!store.has(dataFactory.triple(ex, ex, ex)));
    });
  });

  describe('#has()', function() {
    it('an added quad should be in the store', function() {
      const store = new MemoryStore([dataFactory.triple(ex, ex, ex)]);
      assert(store.has(dataFactory.triple(ex, ex, ex)));
    });
  });

  describe('#match_quads()', function() {
    it('blank pattern should return all quads', function() {
      const store = new MemoryStore([dataFactory.triple(ex, ex, ex)]);
      const results = store.match();
      assert.equal(1, results.length);
      assert(dataFactory.triple(ex, ex, ex).equals(results[0]));
    });
  });

  describe('#query()', function() {
    it('ASK true', function() {
      const store = new MemoryStore([dataFactory.triple(ex, ex, ex)]);
      assert.equal(true, store.query("ASK { ?s ?s ?s }"));
    });

    it('ASK false', function() {
      const store = new MemoryStore();
      assert.equal(false, store.query("ASK { FILTER(false)}"));
    });

    it('CONSTRUCT', function() {
      const store = new MemoryStore([dataFactory.triple(ex, ex, ex)]);
      const results = store.query("CONSTRUCT { ?s ?p ?o } WHERE { ?s ?p ?o }");
      assert.equal(1, results.length);
      assert(dataFactory.triple(ex, ex, ex).equals(results[0]));
    });

    it('SELECT', function() {
      const store = new MemoryStore([dataFactory.triple(ex, ex, ex)]);
      const results = store.query("SELECT ?s WHERE { ?s ?p ?o }");
      assert.equal(1, results.length);
      assert(ex.equals(results[0].get("s")));
    });
  });

  describe('#load()', function() {
    it('load NTriples in the default graph', function() {
      const store = new MemoryStore();
      store.load("<http://example.com> <http://example.com> <http://example.com> .", "application/n-triples");
      assert(store.has(dataFactory.triple(ex, ex, ex)));
    });

    it('load NTriples in an other graph', function() {
      const store = new MemoryStore();
      store.load("<http://example.com> <http://example.com> <http://example.com> .", "application/n-triples", null, ex);
      assert(store.has(dataFactory.quad(ex, ex, ex, ex)));
    });

    it('load Turtle with a base IRI', function() {
      const store = new MemoryStore();
      store.load("<http://example.com> <http://example.com> <> .", "text/turtle", "http://example.com");
      assert(store.has(dataFactory.triple(ex, ex, ex)));
    });

    it('load NQuads', function() {
      const store = new MemoryStore();
      store.load("<http://example.com> <http://example.com> <http://example.com> <http://example.com> .", "application/n-quads");
      assert(store.has(dataFactory.quad(ex, ex, ex, ex)));
    });

    it('load TriG with a base IRI', function() {
      const store = new MemoryStore();
      store.load("GRAPH <> { <http://example.com> <http://example.com> <> }", "application/trig", "http://example.com");
      assert(store.has(dataFactory.quad(ex, ex, ex, ex)));
    });
  });
});
