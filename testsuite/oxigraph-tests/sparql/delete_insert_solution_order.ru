DELETE { <urn:s> <urn:p> ?old }
INSERT { <urn:s> <urn:p> ?new }
WHERE { VALUES (?old ?new) { (1 2) (2 1) } }
