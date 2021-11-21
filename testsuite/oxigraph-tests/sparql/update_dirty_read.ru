PREFIX ex: <http://example.com/>
INSERT DATA { ex:s ex:p 1 . ex:s ex:p 5 };
INSERT { ex:s ex:p ?v } WHERE { VALUES ?o { 1 2 5 6 } FILTER EXISTS { ?s ex:p ?o } BIND(?o + 1 AS ?v) }
