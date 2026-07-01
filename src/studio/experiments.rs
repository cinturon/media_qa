use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperimentState {
    pub variants: Vec<PricingVariant>,
}

impl Default for ExperimentState {
    fn default() -> Self {
        Self {
            variants: vec![
                PricingVariant {
                    id: "control".into(),
                    headline: "Pro at $29/mo".into(),
                    cta: "Start Pro trial".into(),
                },
                PricingVariant {
                    id: "annual_discount".into(),
                    headline: "Pro annual — 2 months free".into(),
                    cta: "Save with annual".into(),
                },
            ],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PricingVariant {
    pub id: String,
    pub headline: String,
    pub cta: String,
}

pub fn assign_variant(state: &ExperimentState, user_id: &str) -> PricingVariant {
    let index = user_id.bytes().map(|b| b as usize).sum::<usize>() % state.variants.len();
    state.variants[index].clone()
}
