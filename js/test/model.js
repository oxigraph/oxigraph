const { MemoryStore } = require('../pkg/oxigraph.js');
const assert = require('assert');
require('../node_modules/@rdfjs/data-model/test/index.js')((new MemoryStore()).dataFactory);
