import { describe, it } from 'node:test';
import assert from 'node:assert';
import { parseAsync, serializeAsync, RdfFormat, namedNode, literal, quad } from '../pkg/index.js';

describe('Async I/O Functions', () => {
  it('parseAsync should parse Turtle data asynchronously', async () => {
    const turtle = `
      @prefix ex: <http://example.com/> .
      ex:subject ex:predicate "object" .
    `;

    const quads = await parseAsync(turtle, RdfFormat.TURTLE, {
      base_iri: 'http://example.com/'
    });

    assert.ok(Array.isArray(quads), 'Result should be an array');
    assert.strictEqual(quads.length, 1, 'Should have one quad');
  });

  it('parseAsync should handle large datasets without blocking', async () => {
    // Generate a large Turtle document
    let turtle = '@prefix ex: <http://example.com/> .\n';
    for (let i = 0; i < 5000; i++) {
      turtle += `ex:subject${i} ex:predicate "object${i}" .\n`;
    }

    const startTime = Date.now();
    const quads = await parseAsync(turtle, RdfFormat.TURTLE);
    const endTime = Date.now();

    assert.strictEqual(quads.length, 5000, 'Should parse all 5000 quads');
    console.log(`Parsed 5000 quads in ${endTime - startTime}ms`);
  });

  it('serializeAsync should serialize quads asynchronously', async () => {
    const quads = [
      quad(
        namedNode('http://example.com/subject'),
        namedNode('http://example.com/predicate'),
        literal('object')
      )
    ];

    const turtle = await serializeAsync(quads, RdfFormat.TURTLE, {
      prefixes: { ex: 'http://example.com/' }
    });

    assert.ok(typeof turtle === 'string', 'Result should be a string');
    assert.ok(turtle.length > 0, 'Result should not be empty');
    assert.ok(turtle.includes('ex:'), 'Should use the prefix');
  });

  it('serializeAsync should handle large datasets without blocking', async () => {
    // Generate a large array of quads
    const quads = [];
    for (let i = 0; i < 5000; i++) {
      quads.push(
        quad(
          namedNode(`http://example.com/subject${i}`),
          namedNode('http://example.com/predicate'),
          literal(`object${i}`)
        )
      );
    }

    const startTime = Date.now();
    const ntriples = await serializeAsync(quads, RdfFormat.N_TRIPLES);
    const endTime = Date.now();

    assert.ok(typeof ntriples === 'string', 'Result should be a string');
    assert.ok(ntriples.split('\n').length >= 5000, 'Should have at least 5000 lines');
    console.log(`Serialized 5000 quads in ${endTime - startTime}ms`);
  });

  it('parseAsync should support all the same options as parse', async () => {
    const ntriples = '<http://example.com/s> <http://example.com/p> "o" .';

    const quads = await parseAsync(ntriples, RdfFormat.N_TRIPLES, {
      base_iri: 'http://example.com/',
      without_named_graphs: true,
      rename_blank_nodes: true,
      lenient: false
    });

    assert.strictEqual(quads.length, 1, 'Should parse one quad');
  });

  it('serializeAsync should support all the same options as serialize', async () => {
    const q = quad(
      namedNode('http://example.com/s'),
      namedNode('http://example.com/p'),
      literal('o')
    );

    const turtle = await serializeAsync([q], RdfFormat.TURTLE, {
      prefixes: { ex: 'http://example.com/' },
      base_iri: 'http://example.com/'
    });

    assert.ok(typeof turtle === 'string', 'Result should be a string');
  });

  it('parseAsync should handle errors gracefully', async () => {
    const invalidTurtle = 'this is not valid turtle';

    await assert.rejects(
      async () => {
        await parseAsync(invalidTurtle, RdfFormat.TURTLE);
      },
      'Should reject with an error for invalid data'
    );
  });

  it('serializeAsync should handle errors gracefully', async () => {
    // Try to serialize invalid data
    await assert.rejects(
      async () => {
        await serializeAsync('not an iterable', RdfFormat.TURTLE);
      },
      'Should reject with an error for invalid input'
    );
  });

  it('parseAsync should parse N3 format', async () => {
    const n3Data = `
      @prefix ex: <http://example.com/> .
      ex:subject ex:predicate "object" .
    `;

    const quads = await parseAsync(n3Data, RdfFormat.N3, {});

    assert.ok(Array.isArray(quads), 'Result should be an array');
    assert.strictEqual(quads.length, 1, 'Should have one quad');
    assert.strictEqual(quads[0].subject.value, 'http://example.com/subject');
    assert.strictEqual(quads[0].predicate.value, 'http://example.com/predicate');
  });

  it('serializeAsync should serialize to N3 format', async () => {
    const quads = [
      quad(
        namedNode('http://example.com/subject'),
        namedNode('http://example.com/predicate'),
        literal('object')
      )
    ];

    const n3 = await serializeAsync(quads, RdfFormat.N3, {
      prefixes: { ex: 'http://example.com/' }
    });

    assert.ok(typeof n3 === 'string', 'Result should be a string');
    assert.ok(n3.length > 0, 'Result should not be empty');
    assert.ok(n3.includes('ex:'), 'Should use the prefix');
  });
});
