use super::*;
use std::collections::{HashMap, VecDeque};
use rand::thread_rng;
use fugue::Distribution;


#[derive(Clone, Copy, PartialEq, Eq)]
enum VisitState {
    Visiting,
    Visited,
}


impl GraphIR{
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



    pub fn ancestral_sample(&self) -> Result<HashMap<u32, f64>, String> {
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
                    return Err(format!("node {id} references missing node {}", param.from_node));
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
    
        if order.len() != self.nodes.len() {
            return Err("graph contains a cycle".into());
        }
    
        let mut rng = rand::thread_rng();
        let mut values = HashMap::with_capacity(self.nodes.len());
    
        for id in order {
            let value = match &self.nodes[&id] {
                NodeIR::Scalar { value, .. } => *value,
    
                NodeIR::Compute {
                    operation, params, ..
                } => {
                    let params: Vec<f64> =
                        params.iter().map(|param| values[&param.from_node]).collect();
    
                    operation.evaluate(&params)?
                }
    
                NodeIR::Random {
                    dist_type, params, ..
                } => {
                    let params: Vec<f64> =
                        params.iter().map(|param| values[&param.from_node]).collect();
    
                    create_distribution(dist_type, &params, id)?.sample(&mut rng)
                }
            };
    
            values.insert(id, value);
        }
    
        Ok(values)
    }
}


type DynDistribution = Box<dyn Distribution<f64>>;

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