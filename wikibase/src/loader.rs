use crate::SERVER;
use chrono::{DateTime, Utc};
use reqwest::header::USER_AGENT;
use reqwest::{Client, Url};
use rudf::model::NamedNode;
use rudf::{GraphSyntax, Repository, RepositoryConnection, Result};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::io::{BufReader, Read};
use std::thread::sleep;
use std::time::Duration;

pub struct WikibaseLoader<R: Repository + Copy> {
    repository: R,
    api_url: Url,
    entity_data_url: Url,
    client: Client,
    namespaces: Vec<u32>,
    frequency: Duration,
    start: DateTime<Utc>,
}

impl<R: Repository + Copy> WikibaseLoader<R> {
    pub fn new(
        repository: R,
        api_url: &str,
        pages_base_url: &str,
        namespaces: &[u32],
        frequency: Duration,
    ) -> Result<Self> {
        Ok(Self {
            repository,
            api_url: Url::parse(api_url)?,
            entity_data_url: Url::parse(&(pages_base_url.to_owned() + "Special:EntityData"))?,
            client: Client::new(),
            namespaces: namespaces.to_vec(),
            start: Utc::now(),
            frequency,
        })
    }

    pub fn initial_loading(&mut self) -> Result<()> {
        println!("Initial loading ");

        self.start = Utc::now();

        for namespace in &self.namespaces {
            let mut parameters = HashMap::default();
            parameters.insert("action".to_owned(), "query".to_owned());
            parameters.insert("list".to_owned(), "allpages".to_owned());
            parameters.insert("apnamespace".to_owned(), namespace.to_string());

            self.api_get_with_continue(parameters, |results| {
                println!("*");
                for page in results
                    .as_object()
                    .unwrap()
                    .get("query")
                    .unwrap()
                    .get("allpages")
                    .unwrap()
                    .as_array()
                    .unwrap()
                {
                    let desc = page.as_object().unwrap();
                    let title = desc.get("title").unwrap().as_str().unwrap();

                    let id = title.split(':').last().unwrap_or(title);

                    match self.get_entity_data(id) {
                        Ok(data) => {
                            self.load_entity_data(
                                &(self.entity_data_url.to_string() + "/" + id),
                                data,
                            )?;
                        }
                        Err(e) => eprintln!("Error while retrieving data for entity {}: {}", id, e),
                    }
                }
                Ok(())
            })?;
        }
        Ok(())
    }

    pub fn update_loop(&mut self) {
        loop {
            let new_start = Utc::now();
            if let Err(e) = self.refresh_step(self.start) {
                eprintln!("WikibaseLoader error: {}", e);
            }
            self.start = new_start;
            let elapsed = (Utc::now() - self.start).to_std().unwrap();
            if elapsed < self.frequency {
                sleep(self.frequency - elapsed);
            }
        }
    }

    fn refresh_step(&self, start: DateTime<Utc>) -> Result<()> {
        let mut seen: HashSet<String> = HashSet::default();

        let mut parameters = HashMap::default();
        parameters.insert("action".to_owned(), "query".to_owned());
        parameters.insert("list".to_owned(), "recentchanges".to_owned());
        parameters.insert(
            "rcnamespace".to_owned(),
            self.namespaces
                .iter()
                .map(|ns| ns.to_string())
                .collect::<Vec<_>>()
                .join("|"),
        );
        parameters.insert("rcend".to_owned(), start.to_rfc2822());
        parameters.insert("rcprop".to_owned(), "title|ids".to_owned());
        parameters.insert("limit".to_owned(), "50".to_owned());

        self.api_get_with_continue(parameters, |results| {
            for change in results
                .as_object()
                .unwrap()
                .get("query")
                .unwrap()
                .get("recentchanges")
                .unwrap()
                .as_array()
                .unwrap()
            {
                let desc = change.as_object().unwrap();
                let title = desc.get("title").unwrap().as_str().unwrap();

                let id = title.split(':').last().unwrap_or(title);
                if seen.contains(id) {
                    continue;
                }
                seen.insert(id.to_owned());

                match self.get_entity_data(id) {
                    Ok(data) => {
                        self.load_entity_data(
                            &(self.entity_data_url.to_string() + "/" + id),
                            data,
                        )?;
                    }
                    Err(e) => eprintln!("Error while retrieving data for entity {}: {}", id, e),
                }
            }
            Ok(())
        })
    }

    fn api_get_with_continue(
        &self,
        mut parameters: HashMap<String, String>,
        mut on_results: impl FnMut(&Value) -> Result<()>,
    ) -> Result<()> {
        loop {
            let results = self.api_get(&mut parameters)?;
            on_results(&results)?;

            if let Some(cont) = results.get("continue") {
                for (k, v) in cont.as_object().unwrap().iter() {
                    parameters.insert(k.to_owned(), v.as_str().unwrap().to_owned());
                }
            } else {
                return Ok(());
            }
        }
    }

    fn api_get(&self, parameters: &mut HashMap<String, String>) -> Result<Value> {
        parameters.insert("format".to_owned(), "json".to_owned());

        Ok(self
            .client
            .get(self.api_url.clone())
            .query(parameters)
            .header(USER_AGENT, SERVER)
            .send()?
            .error_for_status()?
            .json()?)
    }

    fn get_entity_data(&self, id: &str) -> Result<impl Read> {
        Ok(self
            .client
            .get(self.entity_data_url.clone())
            .query(&[("id", id), ("format", "nt"), ("flavor", "dump")])
            .header(USER_AGENT, SERVER)
            .send()?
            .error_for_status()?)
    }

    fn load_entity_data(&self, uri: &str, data: impl Read) -> Result<()> {
        let mut connection = self.repository.connection()?;
        let graph_name = NamedNode::parse(uri)?.into();

        let to_remove = connection
            .quads_for_pattern(None, None, None, Some(Some(&graph_name)))
            .collect::<Result<Vec<_>>>()?;
        for q in to_remove {
            connection.remove(&q)?;
        }

        connection.load_graph(
            BufReader::new(data),
            GraphSyntax::NTriples,
            Some(&NamedNode::parse(uri)?.into()),
            None,
        )
    }
}
