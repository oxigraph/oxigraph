PREFIX : <http://example.com/>
DELETE { ?s :p <<( :s :p <<( _:b :p ?o )>> )>> } WHERE { ?s :p ?o }
