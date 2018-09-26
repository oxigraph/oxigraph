use errors::*;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::sync::RwLock;
use store::numeric_encoder::*;
use store::store::*;

pub type MemoryDataset = StoreDataset<MemoryStore>;
pub type MemoryGraph = StoreDefaultGraph<MemoryStore>;

#[derive(Default)]
pub struct MemoryStore {
    id2str: RwLock<Vec<Vec<u8>>>,
    str2id: RwLock<BTreeMap<Vec<u8>, u64>>,
    graph_indexes: RwLock<BTreeMap<EncodedTerm, MemoryGraphIndexes>>,
}

#[derive(Default)]
struct MemoryGraphIndexes {
    spo: BTreeMap<EncodedTerm, BTreeMap<EncodedTerm, BTreeSet<EncodedTerm>>>,
    pos: BTreeMap<EncodedTerm, BTreeMap<EncodedTerm, BTreeSet<EncodedTerm>>>,
    osp: BTreeMap<EncodedTerm, BTreeMap<EncodedTerm, BTreeSet<EncodedTerm>>>,
}

impl BytesStore for MemoryStore {
    type BytesOutput = Vec<u8>;

    fn insert_bytes(&self, value: &[u8]) -> Result<u64> {
        let mut id2str = self.id2str.write()?;
        let mut str2id = self.str2id.write()?;
        let id = str2id.entry(value.to_vec()).or_insert_with(|| {
            let id = id2str.len() as u64;
            id2str.push(value.to_vec());
            id
        });
        Ok(*id)
    }

    fn get_bytes(&self, id: u64) -> Result<Option<Vec<u8>>> {
        //TODO: use try_from when stable
        let id2str = self.id2str.read()?;
        Ok(if id2str.len() as u64 <= id {
            None
        } else {
            Some(id2str[id as usize].to_owned())
        })
    }
}

impl EncodedQuadsStore for MemoryStore {
    type QuadsIterator = <Vec<Result<EncodedQuad>> as IntoIterator>::IntoIter;
    type QuadsForSubjectIterator = <Vec<Result<EncodedQuad>> as IntoIterator>::IntoIter;
    type QuadsForSubjectPredicateIterator = <Vec<Result<EncodedQuad>> as IntoIterator>::IntoIter;
    type QuadsForSubjectPredicateObjectIterator =
        <Vec<Result<EncodedQuad>> as IntoIterator>::IntoIter;
    type QuadsForSubjectObjectIterator = <Vec<Result<EncodedQuad>> as IntoIterator>::IntoIter;
    type QuadsForPredicateIterator = <Vec<Result<EncodedQuad>> as IntoIterator>::IntoIter;
    type QuadsForPredicateObjectIterator = <Vec<Result<EncodedQuad>> as IntoIterator>::IntoIter;
    type QuadsForObjectIterator = <Vec<Result<EncodedQuad>> as IntoIterator>::IntoIter;
    type QuadsForGraphIterator = <Vec<Result<EncodedQuad>> as IntoIterator>::IntoIter;
    type QuadsForSubjectGraphIterator = <Vec<Result<EncodedQuad>> as IntoIterator>::IntoIter;
    type QuadsForSubjectPredicateGraphIterator =
        <Vec<Result<EncodedQuad>> as IntoIterator>::IntoIter;
    type QuadsForSubjectObjectGraphIterator = <Vec<Result<EncodedQuad>> as IntoIterator>::IntoIter;
    type QuadsForPredicateGraphIterator = <Vec<Result<EncodedQuad>> as IntoIterator>::IntoIter;
    type QuadsForPredicateObjectGraphIterator =
        <Vec<Result<EncodedQuad>> as IntoIterator>::IntoIter;
    type QuadsForObjectGraphIterator = <Vec<Result<EncodedQuad>> as IntoIterator>::IntoIter;

    fn quads(&self) -> Result<<Vec<Result<EncodedQuad>> as IntoIterator>::IntoIter> {
        let mut result = Vec::default();
        for (graph_name, graph) in self.graph_indexes.read()?.iter() {
            for (s, pos) in &graph.spo {
                for (p, os) in pos.iter() {
                    for o in os.iter() {
                        result.push(Ok(EncodedQuad::new(*s, *p, *o, *graph_name)))
                    }
                }
            }
        }
        Ok(result.into_iter())
    }

    fn quads_for_subject(
        &self,
        subject: EncodedTerm,
    ) -> Result<<Vec<Result<EncodedQuad>> as IntoIterator>::IntoIter> {
        let mut result = Vec::default();
        for (graph_name, graph) in self.graph_indexes.read()?.iter() {
            if let Some(pos) = graph.spo.get(&subject) {
                for (p, os) in pos.iter() {
                    for o in os.iter() {
                        result.push(Ok(EncodedQuad::new(subject, *p, *o, *graph_name)))
                    }
                }
            }
        }
        Ok(result.into_iter())
    }

    fn quads_for_subject_predicate(
        &self,
        subject: EncodedTerm,
        predicate: EncodedTerm,
    ) -> Result<<Vec<Result<EncodedQuad>> as IntoIterator>::IntoIter> {
        let mut result = Vec::default();
        for (graph_name, graph) in self.graph_indexes.read()?.iter() {
            if let Some(pos) = graph.spo.get(&subject) {
                if let Some(os) = pos.get(&predicate) {
                    for o in os.iter() {
                        result.push(Ok(EncodedQuad::new(subject, predicate, *o, *graph_name)))
                    }
                }
            }
        }
        Ok(result.into_iter())
    }

    fn quads_for_subject_predicate_object(
        &self,
        subject: EncodedTerm,
        predicate: EncodedTerm,
        object: EncodedTerm,
    ) -> Result<<Vec<Result<EncodedQuad>> as IntoIterator>::IntoIter> {
        let mut result = Vec::default();
        for (graph_name, graph) in self.graph_indexes.read()?.iter() {
            if let Some(pos) = graph.spo.get(&subject) {
                if let Some(os) = pos.get(&predicate) {
                    if os.contains(&object) {
                        result.push(Ok(EncodedQuad::new(
                            subject,
                            predicate,
                            object,
                            *graph_name,
                        )))
                    }
                }
            }
        }
        Ok(result.into_iter())
    }

    fn quads_for_subject_object(
        &self,
        subject: EncodedTerm,
        object: EncodedTerm,
    ) -> Result<<Vec<Result<EncodedQuad>> as IntoIterator>::IntoIter> {
        let mut result = Vec::default();
        for (graph_name, graph) in self.graph_indexes.read()?.iter() {
            if let Some(sps) = graph.osp.get(&object) {
                if let Some(ps) = sps.get(&subject) {
                    for p in ps.iter() {
                        result.push(Ok(EncodedQuad::new(subject, *p, object, *graph_name)))
                    }
                }
            }
        }
        Ok(result.into_iter())
    }

    fn quads_for_predicate(
        &self,
        predicate: EncodedTerm,
    ) -> Result<<Vec<Result<EncodedQuad>> as IntoIterator>::IntoIter> {
        let mut result = Vec::default();
        for (graph_name, graph) in self.graph_indexes.read()?.iter() {
            if let Some(oss) = graph.pos.get(&predicate) {
                for (o, ss) in oss.iter() {
                    for s in ss.iter() {
                        result.push(Ok(EncodedQuad::new(*s, predicate, *o, *graph_name)))
                    }
                }
            }
        }
        Ok(result.into_iter())
    }

    fn quads_for_predicate_object(
        &self,
        predicate: EncodedTerm,
        object: EncodedTerm,
    ) -> Result<<Vec<Result<EncodedQuad>> as IntoIterator>::IntoIter> {
        let mut result = Vec::default();
        for (graph_name, graph) in self.graph_indexes.read()?.iter() {
            if let Some(oss) = graph.pos.get(&predicate) {
                if let Some(ss) = oss.get(&object) {
                    for s in ss.iter() {
                        result.push(Ok(EncodedQuad::new(*s, predicate, object, *graph_name)))
                    }
                }
            }
        }
        Ok(result.into_iter())
    }

    fn quads_for_object(
        &self,
        object: EncodedTerm,
    ) -> Result<<Vec<Result<EncodedQuad>> as IntoIterator>::IntoIter> {
        let mut result = Vec::default();
        for (graph_name, graph) in self.graph_indexes.read()?.iter() {
            if let Some(sps) = graph.osp.get(&object) {
                for (s, ps) in sps.iter() {
                    for p in ps.iter() {
                        result.push(Ok(EncodedQuad::new(*s, *p, object, *graph_name)))
                    }
                }
            }
        }
        Ok(result.into_iter())
    }

    fn quads_for_graph(
        &self,
        graph_name: EncodedTerm,
    ) -> Result<<Vec<Result<EncodedQuad>> as IntoIterator>::IntoIter> {
        let mut result = Vec::default();
        if let Some(graph) = self.graph_indexes.read()?.get(&graph_name) {
            for (s, pos) in &graph.spo {
                for (p, os) in pos.iter() {
                    for o in os.iter() {
                        result.push(Ok(EncodedQuad::new(*s, *p, *o, graph_name)))
                    }
                }
            }
        }
        Ok(result.into_iter())
    }

    fn quads_for_subject_graph(
        &self,
        subject: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> Result<<Vec<Result<EncodedQuad>> as IntoIterator>::IntoIter> {
        let mut result = Vec::default();
        if let Some(graph) = self.graph_indexes.read()?.get(&graph_name) {
            if let Some(pos) = graph.spo.get(&subject) {
                for (p, os) in pos.iter() {
                    for o in os.iter() {
                        result.push(Ok(EncodedQuad::new(subject, *p, *o, graph_name)))
                    }
                }
            }
        }
        Ok(result.into_iter())
    }

    fn quads_for_subject_predicate_graph(
        &self,
        subject: EncodedTerm,
        predicate: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> Result<<Vec<Result<EncodedQuad>> as IntoIterator>::IntoIter> {
        let mut result = Vec::default();
        if let Some(graph) = self.graph_indexes.read()?.get(&graph_name) {
            if let Some(pos) = graph.spo.get(&subject) {
                if let Some(os) = pos.get(&predicate) {
                    for o in os.iter() {
                        result.push(Ok(EncodedQuad::new(subject, predicate, *o, graph_name)))
                    }
                }
            }
        }
        Ok(result.into_iter())
    }

    fn quads_for_subject_object_graph(
        &self,
        subject: EncodedTerm,
        object: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> Result<<Vec<Result<EncodedQuad>> as IntoIterator>::IntoIter> {
        let mut result = Vec::default();
        if let Some(graph) = self.graph_indexes.read()?.get(&graph_name) {
            if let Some(sps) = graph.osp.get(&object) {
                if let Some(ps) = sps.get(&subject) {
                    for p in ps.iter() {
                        result.push(Ok(EncodedQuad::new(subject, *p, object, graph_name)))
                    }
                }
            }
        }
        Ok(result.into_iter())
    }

    fn quads_for_predicate_graph(
        &self,
        predicate: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> Result<<Vec<Result<EncodedQuad>> as IntoIterator>::IntoIter> {
        let mut result = Vec::default();
        if let Some(graph) = self.graph_indexes.read()?.get(&graph_name) {
            if let Some(oss) = graph.pos.get(&predicate) {
                for (o, ss) in oss.iter() {
                    for s in ss.iter() {
                        result.push(Ok(EncodedQuad::new(*s, predicate, *o, graph_name)))
                    }
                }
            }
        }
        Ok(result.into_iter())
    }

    fn quads_for_predicate_object_graph(
        &self,
        predicate: EncodedTerm,
        object: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> Result<<Vec<Result<EncodedQuad>> as IntoIterator>::IntoIter> {
        let mut result = Vec::default();
        if let Some(graph) = self.graph_indexes.read()?.get(&graph_name) {
            if let Some(oss) = graph.pos.get(&predicate) {
                if let Some(ss) = oss.get(&object) {
                    for s in ss.iter() {
                        result.push(Ok(EncodedQuad::new(*s, predicate, object, graph_name)))
                    }
                }
            }
        }
        Ok(result.into_iter())
    }

    fn quads_for_object_graph(
        &self,
        object: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> Result<<Vec<Result<EncodedQuad>> as IntoIterator>::IntoIter> {
        let mut result = Vec::default();
        if let Some(graph) = self.graph_indexes.read()?.get(&graph_name) {
            if let Some(sps) = graph.osp.get(&object) {
                for (s, ps) in sps.iter() {
                    for p in ps.iter() {
                        result.push(Ok(EncodedQuad::new(*s, *p, object, graph_name)))
                    }
                }
            }
        }
        Ok(result.into_iter())
    }

    fn contains(&self, quad: &EncodedQuad) -> Result<bool> {
        Ok(self
            .graph_indexes
            .read()?
            .get(&quad.graph_name)
            .map(|graph| {
                graph
                    .spo
                    .get(&quad.subject)
                    .map(|po| {
                        po.get(&quad.predicate)
                            .map(|o| o.contains(&quad.object))
                            .unwrap_or(false)
                    }).unwrap_or(false)
            }).unwrap_or(false))
    }

    fn insert(&self, quad: &EncodedQuad) -> Result<()> {
        let mut graph_indexes = self.graph_indexes.write()?;
        let graph = graph_indexes
            .entry(quad.graph_name)
            .or_insert_with(MemoryGraphIndexes::default);
        graph
            .spo
            .entry(quad.subject)
            .or_default()
            .entry(quad.predicate)
            .or_default()
            .insert(quad.object);
        graph
            .pos
            .entry(quad.predicate)
            .or_default()
            .entry(quad.object)
            .or_default()
            .insert(quad.subject);
        graph
            .osp
            .entry(quad.object)
            .or_default()
            .entry(quad.subject)
            .or_default()
            .insert(quad.predicate);
        Ok(())
    }

    fn remove(&self, quad: &EncodedQuad) -> Result<()> {
        let mut graph_indexes = self.graph_indexes.write()?;
        let mut empty_graph = false;
        if let Some(graph) = graph_indexes.get_mut(&quad.graph_name) {
            {
                let mut empty_pos = false;
                if let Some(mut pos) = graph.spo.get_mut(&quad.subject) {
                    let mut empty_os = false;
                    if let Some(mut os) = pos.get_mut(&quad.predicate) {
                        os.remove(&quad.object);
                        empty_os = os.is_empty();
                    }
                    if empty_os {
                        pos.remove(&quad.predicate);
                    }
                    empty_pos = pos.is_empty();
                }
                if empty_pos {
                    graph.spo.remove(&quad.subject);
                }
            }

            {
                let mut empty_oss = false;
                if let Some(mut oss) = graph.pos.get_mut(&quad.predicate) {
                    let mut empty_ss = false;
                    if let Some(mut ss) = oss.get_mut(&quad.object) {
                        ss.remove(&quad.subject);
                        empty_ss = ss.is_empty();
                    }
                    if empty_ss {
                        oss.remove(&quad.object);
                    }
                    empty_oss = oss.is_empty();
                }
                if empty_oss {
                    graph.pos.remove(&quad.predicate);
                }
            }

            {
                let mut empty_sps = false;
                if let Some(mut sps) = graph.osp.get_mut(&quad.object) {
                    let mut empty_ps = false;
                    if let Some(mut ps) = sps.get_mut(&quad.subject) {
                        ps.remove(&quad.predicate);
                        empty_ps = ps.is_empty();
                    }
                    if empty_ps {
                        sps.remove(&quad.subject);
                    }
                    empty_sps = sps.is_empty();
                }
                if empty_sps {
                    graph.osp.remove(&quad.object);
                }
            }

            empty_graph = graph.spo.is_empty();
        }
        if empty_graph {
            graph_indexes.remove(&quad.graph_name);
        }
        Ok(())
    }
}
