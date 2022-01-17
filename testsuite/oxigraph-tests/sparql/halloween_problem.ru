PREFIX ex: <http://example.com/>
INSERT DATA { ex:s ex:salary 1200 . ex:s2 ex:salary 1250 . ex:boss ex:salary 1600 . };
DELETE { ?s ex:salary ?o } INSERT { ?s ex:salary ?v } WHERE { ?s ex:salary ?o FILTER(?o < 1500) BIND(?o + 100 AS ?v) }
