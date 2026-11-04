//! Single-site MH for mixed models: symmetric real walks and discrete prior proposals
//! with their Hastings correction. Rejected invalid executions leave the chain intact.
use super::{CompiledGraph, graph_checks::PosteriorValues};
use fugue::{
    Address, Choice, ChoiceValue, DiminishingAdaptation, Distribution, Handler, Normal, Trace,
};
use rand::{Rng, rngs::StdRng};

struct Replay<'a> {
    base: &'a Trace,
    trace: Trace,
    selected: Option<&'a Address>,
    rng: &'a mut StdRng,
    scale: f64,
    correction: &'a mut f64,
}
impl Replay<'_> {
    fn record(&mut self, addr: &Address, value: ChoiceValue, logp: f64) {
        self.trace.log_prior += logp;
        self.trace.choices.insert(
            addr.clone(),
            Choice {
                addr: addr.clone(),
                value,
                logp,
            },
        );
    }
}
macro_rules! discrete {
    ($sample:ident,$observe:ident,$ty:ty,$variant:ident,$getter:ident) => {
        fn $sample(&mut self, addr: &Address, dist: &dyn Distribution<$ty>) -> $ty {
            let old = self
                .base
                .$getter(addr)
                .expect("static graph trace includes every latent");
            let value = if self.selected == Some(addr) {
                let value = dist.sample(self.rng);
                *self.correction += dist.log_prob(&old) - dist.log_prob(&value);
                value
            } else {
                old
            };
            self.record(addr, ChoiceValue::$variant(value), dist.log_prob(&value));
            value
        }
        fn $observe(&mut self, _: &Address, dist: &dyn Distribution<$ty>, value: $ty) {
            self.trace.log_likelihood += dist.log_prob(&value);
        }
    };
}
impl Handler for Replay<'_> {
    fn on_sample_f64(&mut self, addr: &Address, dist: &dyn Distribution<f64>) -> f64 {
        let old = self.base.get_f64(addr).expect("real choice");
        let value = if self.selected == Some(addr) {
            Normal::new(old, self.scale)
                .expect("finite positive proposal scale")
                .sample(self.rng)
        } else {
            old
        };
        self.record(addr, ChoiceValue::F64(value), dist.log_prob(&value));
        value
    }
    fn on_observe_f64(&mut self, _: &Address, dist: &dyn Distribution<f64>, value: f64) {
        self.trace.log_likelihood += dist.log_prob(&value);
    }
    discrete!(on_sample_u64, on_observe_u64, u64, U64, get_u64);
    discrete!(on_sample_usize, on_observe_usize, usize, Usize, get_usize);
    fn on_sample_bool(&mut self, _: &Address, _: &dyn Distribution<bool>) -> bool {
        unreachable!("no boolean distributions")
    }
    fn on_observe_bool(&mut self, _: &Address, _: &dyn Distribution<bool>, _: bool) {
        unreachable!("no boolean distributions")
    }
    fn on_factor(&mut self, v: f64) {
        self.trace.log_factors += v;
    }
    fn finish(self) -> Trace {
        self.trace
    }
}
pub(super) fn step(
    rng: &mut StdRng,
    compiled: &CompiledGraph,
    current: &Trace,
    adaptation: &mut DiminishingAdaptation,
) -> (Result<PosteriorValues, String>, Trace) {
    let mut correction = 0.;
    let (values, scored) = fugue::runtime::handler::run(
        Replay {
            base: current,
            trace: Trace::default(),
            selected: None,
            rng,
            scale: 1.,
            correction: &mut correction,
        },
        compiled.inference_model(),
    );
    if values.is_err() || current.choices.is_empty() {
        return (values, scored);
    }
    let mut sites = current.choices.keys().collect::<Vec<_>>();
    sites.sort_by(|a, b| a.0.cmp(&b.0));
    let site = sites[rng.gen_range(0..sites.len())];
    let scale = adaptation.get_scale(site).clamp(1e-6, 1e6);
    let (proposed, trace) = fugue::runtime::handler::run(
        Replay {
            base: current,
            trace: Trace::default(),
            selected: Some(site),
            rng,
            scale,
            correction: &mut correction,
        },
        compiled.inference_model(),
    );
    let alpha = trace.total_log_weight() - scored.total_log_weight() + correction;
    let accept =
        proposed.is_ok() && alpha.is_finite() && (alpha >= 0. || rng.r#gen::<f64>().ln() < alpha);
    adaptation.update(site, accept);
    if accept {
        (proposed, trace)
    } else {
        (values, scored)
    }
}
