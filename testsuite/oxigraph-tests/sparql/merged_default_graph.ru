INSERT { <urn:report> <urn:total> ?total }
USING <merged_graph_1.ttl>
USING <merged_graph_2.ttl>
WHERE {
    SELECT (SUM(?value) AS ?total)
    WHERE { ?s <urn:p> ?value }
}
