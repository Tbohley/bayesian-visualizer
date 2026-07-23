use super::*;
use fugue::{pure, Address, Distribution, Model, ModelExt};
use std::collections::HashMap;

/// The values produced by one execution of a compiled graph model, keyed by
/// [`GraphNode`](crate::nodes::GraphNode) ID.
pub type ModelValues = HashMap<u32, f64>;

/// Model execution can fail after an upstream random value has been sampled.
/// For example, that value may be used as an invalid standard deviation by a
/// downstream distribution. Keeping the error in the model's result lets us
/// build the complete hierarchy without panicking inside a `bind` closure.
pub type GraphModel = Model<Result<ModelValues, String>>;

#[derive(Clone, Copy, PartialEq, Eq)]
enum VisitState {
    Visiting,
    Visited,
}

impl GraphIR {
    pub fn check_cycles(&self) -> Result<(), Vec<u32>> {
        fn params(node: &NodeIR) -> &[ParamIR] {
            match node {
                NodeIR::Random { params, .. } => params,
                NodeIR::Compute { params, .. } => params,
                NodeIR::Scalar { .. } => &[],
            }
        }

        fn visit(
            node_id: u32,
            graph: &GraphIR,
            states: &mut HashMap<u32, VisitState>,
            stack: &mut Vec<u32>,
        ) -> Result<(), Vec<u32>> {
            if states.get(&node_id) == Some(&VisitState::Visiting) {
                let cycle_start = stack
                    .iter()
                    .position(|id| *id == node_id)
                    .expect("visiting node should be in DFS stack");

                return Err(stack[cycle_start..].to_vec());
            }

            if states.get(&node_id) == Some(&VisitState::Visited) {
                return Ok(());
            }

            states.insert(node_id, VisitState::Visiting);
            stack.push(node_id);

            if let Some(node) = graph.nodes.get(&node_id) {
                for param in params(node) {
                    visit(param.from_node, graph, states, stack)?;
                }
            }

            stack.pop();
            states.insert(node_id, VisitState::Visited);

            Ok(())
        }

        let mut states = HashMap::new();
        let mut stack = Vec::new();

        for &node_id in self.nodes.keys() {
            visit(node_id, self, &mut states, &mut stack)?;
        }

        Ok(())
    }

    pub fn topological_sort(&self) -> Result<Vec<u32>, String> {
        let mut indegrees = HashMap::new();
        let mut children: HashMap<u32, Vec<u32>> = HashMap::new();

        for (&id, node) in &self.nodes {
            let params: &[ParamIR] = match node {
                NodeIR::Random { params, .. } | NodeIR::Compute { params, .. } => params.as_slice(),
                NodeIR::Scalar { .. } => &[],
            };
            indegrees.insert(id, params.len());
            for param in params {
                if !self.nodes.contains_key(&param.from_node) {
                    return Err(format!(
                        "node {id} references missing node {}",
                        param.from_node
                    ));
                }
                children.entry(param.from_node).or_default().push(id);
            }
        }
        let mut ready: Vec<u32> = indegrees
            .iter()
            .filter_map(|(&id, &degree)| (degree == 0).then_some(id))
            .collect();

        let mut order = Vec::with_capacity(self.nodes.len());

        while let Some(id) = ready.pop() {
            order.push(id);

            for &child in children.get(&id).into_iter().flatten() {
                let degree = indegrees
                    .get_mut(&child)
                    .expect("child should have an indegree");

                *degree -= 1;

                if *degree == 0 {
                    ready.push(child);
                }
            }
        }
        Ok(order)
    }

    pub fn ancestral_sample(&self) -> Result<HashMap<u32, f64>, String> {
        let model = self.create_model()?;
        let mut rng = rand::thread_rng();
        let (result, _trace) = fugue::runtime::handler::run(
            fugue::runtime::interpreters::PriorHandler {
                rng: &mut rng,
                trace: fugue::runtime::trace::Trace::default(),
            },
            model,
        );
        result
    }

    /// Compile this graph into a single hierarchical Fugue model.
    ///
    /// Nodes are appended in topological order. Scalar and compute nodes are
    /// deterministic `pure` steps; random nodes are genuine Fugue sample sites.
    /// Since each step is attached with `bind`, a downstream distribution is
    /// not constructed until all of its upstream random values have actually
    /// been sampled by the model's handler.
    ///
    /// The outer `Result` reports graph-structure errors found while compiling.
    /// The model's inner `Result` reports value-dependent errors found while it
    /// is running, such as division by zero or invalid distribution parameters.
    pub fn create_model(&self) -> Result<GraphModel, String> {
        if let Err(cycle) = self.check_cycles() {
            return Err(format!(
                "graph contains a cycle including node IDs: {cycle:?}"
            ));
        }

        let order = self.topological_sort()?;
        if order.len() != self.nodes.len() {
            return Err("graph contains a cycle".to_string());
        }

        let mut model: GraphModel = pure(Ok(HashMap::with_capacity(self.nodes.len())));

        for id in order {
            let node = self
                .nodes
                .get(&id)
                .cloned()
                .ok_or_else(|| format!("topological order references missing node {id}"))?;

            model = model.bind(move |result| {
                let mut values = match result {
                    Ok(values) => values,
                    Err(error) => return pure(Err(error)),
                };

                match node {
                    NodeIR::Scalar { value, .. } => {
                        values.insert(id, value);
                        pure(Ok(values))
                    }
                    NodeIR::Compute {
                        operation, params, ..
                    } => {
                        let params = match resolve_params(id, &params, &values) {
                            Ok(params) => params,
                            Err(error) => return pure(Err(error)),
                        };

                        match operation.evaluate(&params) {
                            Ok(value) => {
                                values.insert(id, value);
                                pure(Ok(values))
                            }
                            Err(error) => pure(Err(format!("compute error at node {id}: {error}"))),
                        }
                    }
                    NodeIR::Random {
                        label,
                        dist_type,
                        params,
                        ..
                    } => {
                        let params = match resolve_params(id, &params, &values) {
                            Ok(params) => params,
                            Err(error) => return pure(Err(error)),
                        };
                        let distribution = match create_distribution(&dist_type, &params, id) {
                            Ok(distribution) => distribution,
                            Err(error) => return pure(Err(error)),
                        };

                        let address =
                            Address(format!("{}#{id}", label.as_deref().unwrap_or("node")));

                        sample_dyn_f64(address, distribution).bind(move |value| {
                            values.insert(id, value);
                            pure(Ok(values))
                        })
                    }
                }
            });
        }

        Ok(model)
    }

    /// Render the dynamic model as source-like Rust for debugging.
    ///
    /// This mirrors the bind chain built by [`Self::create_model`]; it is not
    /// intended to be compiled independently.
    pub fn bind_debug_string(&self) -> Result<String, String> {
        if let Err(cycle) = self.check_cycles() {
            return Err(format!(
                "graph contains a cycle including node IDs: {cycle:?}"
            ));
        }

        let order = self.topological_sort()?;
        if order.len() != self.nodes.len() {
            return Err("graph contains a cycle".to_string());
        }

        let mut output = String::from("pure(Ok(HashMap::<u32, f64>::new()))\n");

        for id in order {
            let node = self
                .nodes
                .get(&id)
                .ok_or_else(|| format!("topological order references missing node {id}"))?;

            output.push_str("    .bind(move |result| {\n");
            output.push_str("        let mut values = result?;\n");

            match node {
                NodeIR::Scalar { value, .. } => {
                    output.push_str(&format!(
                        "        values.insert({id}, {value:?});\n\
                         \x20       pure(Ok(values))\n"
                    ));
                }
                NodeIR::Compute {
                    operation, params, ..
                } => {
                    output.push_str(&format!(
                        "        let params = {};\n\
                         \x20       let value = Operation::{operation:?}.evaluate(&params)?;\n\
                         \x20       values.insert({id}, value);\n\
                         \x20       pure(Ok(values))\n",
                        debug_param_values(params)
                    ));
                }
                NodeIR::Random {
                    label,
                    dist_type,
                    params,
                    ..
                } => {
                    let address = format!("{}#{id}", label.as_deref().unwrap_or("node"));
                    output.push_str(&format!(
                        "        let params = {};\n\
                         \x20       let dist = create_distribution({dist_type:?}, &params, {id})?;\n\
                         \x20       sample_dyn_f64(Address({address:?}.into()), dist)\n\
                         \x20           .bind(move |value| {{\n\
                         \x20               values.insert({id}, value);\n\
                         \x20               pure(Ok(values))\n\
                         \x20           }})\n",
                        debug_param_values(params)
                    ));
                }
            }

            output.push_str("    })\n");
        }

        Ok(output)
    }
}

fn debug_param_values(params: &[ParamIR]) -> String {
    let values = params
        .iter()
        .map(|param| format!("values[&{}]", param.from_node))
        .collect::<Vec<_>>()
        .join(", ");
    format!("vec![{values}]")
}

fn resolve_params(
    node_id: u32,
    params: &[ParamIR],
    values: &ModelValues,
) -> Result<Vec<f64>, String> {
    params
        .iter()
        .map(|param| {
            values.get(&param.from_node).copied().ok_or_else(|| {
                format!(
                    "node {node_id} parameter references unavailable node {}",
                    param.from_node
                )
            })
        })
        .collect()
}

type DynDistribution = Box<dyn Distribution<f64>>;

fn sample_dyn_f64(addr: Address, dist: DynDistribution) -> Model<f64> {
    Model::SampleF64 {
        addr,
        dist,
        k: Box::new(pure),
    }
}

fn boxed<D: Distribution<f64> + 'static>(
    result: FugueResult<D>,
    node_id: u32,
) -> Result<DynDistribution, String> {
    result
        .map(|dist| Box::new(dist) as DynDistribution)
        .map_err(|error| format!("{error} at node {node_id}"))
}

fn create_distribution(
    dist_type: &str,
    params: &[f64],
    node_id: u32,
) -> Result<DynDistribution, String> {
    match (dist_type, params) {
        ("Normal", &[mu, sigma]) => boxed(Normal::new(mu, sigma), node_id),
        ("Uniform", &[low, high]) => boxed(Uniform::new(low, high), node_id),
        ("Beta", &[alpha, beta]) => boxed(Beta::new(alpha, beta), node_id),
        ("Exponential", &[rate]) => boxed(Exponential::new(rate), node_id),
        ("Gamma", &[shape, rate]) => boxed(Gamma::new(shape, rate), node_id),
        ("LogNormal", &[mu, sigma]) => boxed(LogNormal::new(mu, sigma), node_id),
        (name, _) => Err(format!("invalid parameters for {name} at node {node_id}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fugue::runtime::{handler::run, interpreters::PriorHandler, trace::Trace};
    use rand::{rngs::StdRng, SeedableRng};

    fn param(from_node: u32) -> ParamIR {
        ParamIR {
            from_node,
            param_name: None,
        }
    }

    #[test]
    fn create_model_builds_dependent_random_sites() {
        let mut graph = GraphIR::new();
        graph.nodes.insert(1, NodeIR::Scalar { id: 1, value: 0.0 });
        graph.nodes.insert(2, NodeIR::Scalar { id: 2, value: 1.0 });
        graph.nodes.insert(
            3,
            NodeIR::Random {
                id: 3,
                label: Some("mu".to_string()),
                dist_type: "Normal".to_string(),
                params: vec![param(1), param(2)],
            },
        );
        graph.nodes.insert(
            4,
            NodeIR::Scalar {
                id: 4,
                value: 1.0e-12,
            },
        );
        graph.nodes.insert(
            5,
            NodeIR::Random {
                id: 5,
                label: Some("x".to_string()),
                dist_type: "Normal".to_string(),
                params: vec![param(3), param(4)],
            },
        );

        let debug_code = graph
            .bind_debug_string()
            .expect("debug rendering should work");
        assert!(debug_code.contains("Address(\"mu#3\".into())"));
        assert!(debug_code.contains("Address(\"x#5\".into())"));
        assert!(debug_code.contains(".bind(move |value|"));

        let model = graph.create_model().expect("valid graph should compile");
        let mut rng = StdRng::seed_from_u64(42);
        let (values, trace) = run(
            PriorHandler {
                rng: &mut rng,
                trace: Trace::default(),
            },
            model,
        );
        let values = values.expect("valid parameters should execute");

        let mu = trace
            .get_f64(&Address("mu#3".to_string()))
            .expect("mu should be a Fugue sample site");
        let x = trace
            .get_f64(&Address("x#5".to_string()))
            .expect("x should be a Fugue sample site");

        assert_eq!(values[&3], mu);
        assert_eq!(values[&5], x);
        assert!((x - mu).abs() < 1.0e-8);
    }

    #[test]
    fn create_model_rejects_cycles() {
        let mut graph = GraphIR::new();
        graph.nodes.insert(
            1,
            NodeIR::Compute {
                id: 1,
                operation: crate::nodes::Operation::Exponential,
                params: vec![param(2)],
            },
        );
        graph.nodes.insert(
            2,
            NodeIR::Compute {
                id: 2,
                operation: crate::nodes::Operation::Exponential,
                params: vec![param(1)],
            },
        );

        assert!(graph.create_model().is_err());
    }
}
