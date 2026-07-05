use std::hash::{Hash, Hasher};

#[derive(Debug, Eq, PartialEq, Hash, Clone)]
pub struct ColorInterpretation {
    ranging: ColorRanging,
    colormap: ColorMap,
}

impl Default for ColorInterpretation {
    fn default() -> Self {
        todo!()
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum ColorRanging {
    MinMax,
    Percentile(f32, f32),
    Manual(f32, f32),
    GdalInterpretation,
}

impl Eq for ColorRanging {}

impl Hash for ColorRanging {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            ColorRanging::MinMax => {}
            ColorRanging::Percentile(a, b) => {
                a.to_bits().hash(state);
                b.to_bits().hash(state);
            }
            ColorRanging::Manual(a, b) => {
                a.to_bits().hash(state);
                b.to_bits().hash(state);
            }
            ColorRanging::GdalInterpretation => {}
        }
    }
}

#[derive(Debug, Eq, PartialEq, Hash, Clone)]
pub struct ColorMap;
