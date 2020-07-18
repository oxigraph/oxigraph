const { MemoryStore } = require('../pkg/oxigraph.js')
require('../node_modules/@rdfjs/data-model/test/index.js')((new MemoryStore()).dataFactory)
