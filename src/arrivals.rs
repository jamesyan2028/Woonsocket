use rand::rngs::StdRng;
use rand::SeedableRng;
use rand_distr::{Distribution, Exp};

pub enum Arrivals {
    Constant { next_ns: f64, step_ns: f64 },
    Poisson { next_ns: f64, gap: Exp<f64>, rng: StdRng },
}

impl Arrivals {

    pub fn constant(step_ns: f64, offset_ns: f64) -> Self {
        assert!(step_ns > 0.0, "interval must be positive");
        Arrivals::Constant { next_ns: offset_ns, step_ns }
    }

    pub fn poisson(mean_ns: f64) -> Self {
        assert!(mean_ns > 0.0, "interval must be positive");
        let gap = Exp::new(1.0 / mean_ns).expect("valid exponential rate");
        let mut rng = StdRng::from_entropy();
        let first = gap.sample(&mut rng);
        Arrivals::Poisson { next_ns: first, gap, rng }
    }
}

impl Iterator for Arrivals {
    type Item = u64;

    fn next(&mut self) -> Option<u64> {
        match self {
            Arrivals::Constant { next_ns, step_ns } => {
                let t = *next_ns;
                *next_ns += *step_ns;
                Some(t as u64)
            }
            Arrivals::Poisson { next_ns, gap, rng } => {
                let t = *next_ns;
                *next_ns += gap.sample(rng);
                Some(t as u64)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Arrivals;

    #[test]
    fn constant_is_evenly_spaced() {
        let times: Vec<u64> = Arrivals::constant(1000.0, 250.0).take(4).collect();
        assert_eq!(times, vec![250, 1250, 2250, 3250]);
    }

    #[test]
    fn poisson_has_the_right_mean_and_spread() {
        let n = 200_000;
        let times: Vec<u64> = Arrivals::poisson(1000.0).take(n).collect();
        let gaps: Vec<f64> = times.windows(2).map(|w| (w[1] - w[0]) as f64).collect();
        let mean = gaps.iter().sum::<f64>() / gaps.len() as f64;
        let var = gaps.iter().map(|g| (g - mean).powi(2)).sum::<f64>() / gaps.len() as f64;
        assert!((mean - 1000.0).abs() < 20.0, "mean gap {mean}");
        assert!((var.sqrt() - 1000.0).abs() < 30.0, "std dev {}", var.sqrt());
    }
}
