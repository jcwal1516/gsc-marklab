use std::collections::{BTreeMap, BTreeSet};
use std::str::FromStr;

use marklab_workflow::{ArtifactId, ArtifactRef, ContentDigest};

use crate::{
    AssayMarkDeclaration, AssayMarkValues, CellId, CoordinateFrame, CoordinateFrameId,
    CoordinateRegistry, CoordinateSpace, CoordinateUnit, GlobalMoranWeightPolicy, MarkTable,
    MeasurementStatus, ObservationWindow2D, ObservationWindowLimits, Result, ScalarMarkColumn,
    ScalarMarkId, SlideId, SpatialAxis,
};

use super::{
    invalid,
    model::{Channel, ChannelKind, Recipe, Slide},
};

pub(super) const MAXIMUM_RECIPE_BYTES: usize = 16 * 1024 * 1024;

pub(super) struct PreparedStudy {
    pub recipe: Recipe,
    pub recipe_digest: ContentDigest,
    pub slides: Vec<PreparedSlide>,
    pub selection: Vec<ScalarMarkId>,
    pub weight_policy: GlobalMoranWeightPolicy,
}

pub(super) struct PreparedSlide {
    pub source: Slide,
    pub source_artifact: ArtifactRef,
    pub panel_artifact: ArtifactRef,
    pub table: MarkTable,
    pub x: Vec<f64>,
    pub y: Vec<f64>,
    pub frame: CoordinateFrameId,
    pub window: ObservationWindow2D,
}

impl PreparedStudy {
    pub fn from_json(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > MAXIMUM_RECIPE_BYTES {
            return Err(invalid("recipe exceeds 16 MiB"));
        }
        let mut recipe: Recipe = serde_json::from_slice(bytes)?;
        validate_recipe(&recipe, bytes.len())?;
        let weight_policy = match recipe.design.weight_policy.as_str() {
            "binary_symmetric" => GlobalMoranWeightPolicy::BinarySymmetric,
            "row_standardized" => GlobalMoranWeightPolicy::RowStandardized,
            _ => {
                return Err(invalid(
                    "weight_policy must be binary_symmetric or row_standardized",
                ))
            }
        };
        let selection = recipe
            .design
            .selected_channels
            .iter()
            .map(|id| ScalarMarkId::new(id).map_err(|error| invalid(error.to_string())))
            .collect::<Result<Vec<_>>>()?;
        let sources = std::mem::take(&mut recipe.slides);
        let mut slides = sources
            .into_iter()
            .map(|source| prepare_slide(source, &recipe))
            .collect::<Result<Vec<_>>>()?;
        slides.sort_by(|left, right| left.source.slide_id.cmp(&right.source.slide_id));
        Ok(Self {
            recipe,
            recipe_digest: ContentDigest::from_bytes(bytes),
            slides,
            selection,
            weight_policy,
        })
    }
}

fn validate_recipe(recipe: &Recipe, byte_count: usize) -> Result<()> {
    if recipe.format != "marklab.multiplex_study_recipe"
        || recipe.version != 1
        || !label(&recipe.study_id)
    {
        return Err(invalid(
            "expected a named marklab.multiplex_study_recipe version 1",
        ));
    }
    let design = &recipe.design;
    if design.missingness != "per_channel_complete_case"
        || design.patient_reduction != "equal_slide_mean"
        || design.exchangeability != "independent_patients"
    {
        return Err(invalid("this profile requires per_channel_complete_case, equal_slide_mean and independent_patients"));
    }
    if !design.radius_um.is_finite()
        || design.radius_um <= 0.0
        || !(design.alpha > 0.0 && design.alpha < 1.0)
        || design.permutations == 0
        || design.permutations > 100_000
    {
        return Err(invalid("invalid radius, alpha or permutation count"));
    }
    if !label(&design.group_a) || !label(&design.group_b) || design.group_a == design.group_b {
        return Err(invalid("two distinct bounded group labels are required"));
    }
    if recipe.channels.is_empty()
        || recipe.channels.len() > 64
        || recipe.slides.is_empty()
        || recipe.slides.len() > 1024
    {
        return Err(invalid(
            "this profile admits 1..=64 channels and 1..=1024 slides",
        ));
    }
    let channels = recipe
        .channels
        .iter()
        .map(|channel| (&channel.id, channel))
        .collect::<BTreeMap<_, _>>();
    if channels.len() != recipe.channels.len() {
        return Err(invalid("duplicate channel identity"));
    }
    let selected = &design.selected_channels;
    if selected.is_empty()
        || selected.len() > 32
        || selected.iter().collect::<BTreeSet<_>>().len() != selected.len()
    {
        return Err(invalid(
            "select 1..=32 distinct quantitative or binary channels",
        ));
    }
    for id in selected {
        if channels
            .get(id)
            .is_none_or(|channel| channel.kind == ChannelKind::Categorical)
        {
            return Err(invalid(format!(
                "selected channel {id} is absent or nominal"
            )));
        }
    }
    for channel in &recipe.channels {
        declaration(channel)?;
        if channel.kind != ChannelKind::Categorical && !channel.levels.is_empty() {
            return Err(invalid("only categorical channels declare a codebook"));
        }
    }
    let limits = recipe.limits;
    if limits.maximum_rows_per_slide == 0
        || limits.maximum_rows_per_slide > 1_000_000
        || limits.maximum_total_rows == 0
        || limits.maximum_total_rows > 2_000_000
        || limits.maximum_directed_edges == 0
        || limits.maximum_directed_edges > 8_000_000
        || limits.maximum_edge_evaluations == 0
        || limits.maximum_edge_evaluations > 100_000_000
        || limits.maximum_memory_bytes == 0
        || limits.maximum_memory_bytes > 2 * 1024 * 1024 * 1024
    {
        return Err(invalid("resource limits exceed the fixed admitted bounds"));
    }
    let mut total_rows = 0usize;
    let mut slide_ids = BTreeSet::new();
    let mut cell_ids = BTreeSet::new();
    let mut patient_groups = BTreeMap::new();
    for slide in &recipe.slides {
        if !label(&slide.slide_id)
            || !label(&slide.patient_id)
            || !slide_ids.insert(&slide.slide_id)
        {
            return Err(invalid(
                "slide and patient identities must be bounded; slide IDs must be unique",
            ));
        }
        if slide.group != design.group_a && slide.group != design.group_b {
            return Err(invalid(format!(
                "slide {} has an undeclared group",
                slide.slide_id
            )));
        }
        if patient_groups
            .insert(&slide.patient_id, &slide.group)
            .is_some_and(|group| group != &slide.group)
        {
            return Err(invalid("one patient has contradictory group labels"));
        }
        let rows = slide.cell_ids.len();
        if slide.cell_ids.iter().any(|id| !cell_ids.insert(id)) {
            return Err(invalid("cell identities must be unique across the study; qualify source-local IDs with their slide"));
        }
        if rows > limits.maximum_rows_per_slide || rows != slide.coordinates_um.len() {
            return Err(invalid(format!(
                "slide {} has mismatched or excessive coordinate rows",
                slide.slide_id
            )));
        }
        total_rows = total_rows
            .checked_add(rows)
            .ok_or_else(|| invalid("row count overflow"))?;
        if slide.observations.len() != channels.len()
            || channels
                .keys()
                .any(|id| !slide.observations.contains_key(*id))
        {
            return Err(invalid("every slide must carry exactly the declared channels, using null for missing observations"));
        }
    }
    if total_rows > limits.maximum_total_rows {
        return Err(invalid("total cell rows exceed the study limit"));
    }
    for group in [&design.group_a, &design.group_b] {
        if patient_groups
            .values()
            .filter(|value| *value == &group)
            .count()
            < 2
        {
            return Err(invalid(
                "each group requires at least two declared independent patients",
            ));
        }
    }
    let inference_work = patient_groups
        .len()
        .checked_mul(selected.len())
        .and_then(|work| work.checked_mul(2))
        .and_then(|work| work.checked_mul(design.permutations + 1))
        .ok_or_else(|| invalid("patient inference work overflow"))?;
    if inference_work > 100_000_000 {
        return Err(invalid(
            "patient inference exceeds 100 million endpoint evaluations",
        ));
    }
    // Conservative admission accounting, not a measured allocator/RSS guarantee. Includes JSON
    // decoding, retained source + typed columns, coordinates, graph digest buffers and outputs.
    let retained = byte_count
        .checked_mul(16)
        .and_then(|bytes| {
            total_rows
                .checked_mul(128 + recipe.channels.len() * 32)
                .and_then(|rows| bytes.checked_add(rows))
        })
        .and_then(|bytes| {
            limits
                .maximum_directed_edges
                .checked_mul(96)
                .and_then(|edges| bytes.checked_add(edges))
        })
        .and_then(|bytes| bytes.checked_add(32 * 1024 * 1024))
        .ok_or_else(|| invalid("retained-memory estimate overflow"))?;
    if retained > limits.maximum_memory_bytes {
        return Err(invalid(format!(
            "retained-memory estimate {retained} exceeds budget {}",
            limits.maximum_memory_bytes
        )));
    }
    Ok(())
}

fn prepare_slide(source: Slide, recipe: &Recipe) -> Result<PreparedSlide> {
    let source_artifact = ArtifactRef::from_bytes(
        "application/vnd.marklab.multiplex-slide+json;version=1",
        &serde_json::to_vec(&source)?,
    )
    .map_err(|error| invalid(error.to_string()))?;
    let frame = CoordinateFrameId::new(&source.coordinate_frame_id)
        .map_err(|error| invalid(error.to_string()))?;
    let declared = CoordinateFrame::new(
        frame.clone(),
        vec![SpatialAxis::X, SpatialAxis::Y],
        CoordinateUnit::Micrometer,
        CoordinateSpace::Physical,
    )
    .map_err(|error| invalid(error.to_string()))?;
    let registry = CoordinateRegistry::new(vec![declared], vec![], vec![], vec![])
        .map_err(|error| invalid(error.to_string()))?;
    let window = ObservationWindow2D::from_geojson_str(
        &serde_json::to_string(&source.window)?,
        ObservationWindowLimits::new(1024 * 1024, 1024, 4096, 100_000, 1_000_000)
            .map_err(|error| invalid(error.to_string()))?,
    )
    .and_then(|window| window.with_coordinate_frame(&registry, frame.clone()))
    .map_err(|error| invalid(format!("slide {} window: {error}", source.slide_id)))?;
    let ids = source
        .cell_ids
        .iter()
        .map(|id| CellId::new(id).map_err(|error| invalid(error.to_string())))
        .collect::<Result<Vec<_>>>()?;
    let mut columns = Vec::with_capacity(recipe.channels.len());
    for channel in &recipe.channels {
        let observations = &source.observations[&channel.id];
        let values = match channel.kind {
            ChannelKind::Continuous => AssayMarkValues::Continuous(observations.clone()),
            ChannelKind::Binary => AssayMarkValues::Binary(
                observations
                    .iter()
                    .map(|value| match value {
                        None => Ok(None),
                        Some(0.0) => Ok(Some(false)),
                        Some(1.0) => Ok(Some(true)),
                        _ => Err(invalid(format!(
                            "channel {} requires binary zero/one or null",
                            channel.id
                        ))),
                    })
                    .collect::<Result<Vec<_>>>()?,
            ),
            ChannelKind::Categorical => AssayMarkValues::Categorical {
                levels: channel.levels.clone(),
                values: observations
                    .iter()
                    .map(|value| match value {
                        None => Ok(None),
                        Some(value)
                            if value.is_finite()
                                && *value >= 0.0
                                && *value < channel.levels.len() as f64
                                && value.fract() == 0.0 =>
                        {
                            Ok(Some(*value as u32))
                        }
                        _ => Err(invalid(format!(
                            "channel {} has an invalid category code",
                            channel.id
                        ))),
                    })
                    .collect::<Result<Vec<_>>>()?,
            },
        };
        columns.push(
            ScalarMarkColumn::assay(declaration(channel)?, values).map_err(|error| {
                invalid(format!(
                    "slide {} channel {}: {error}",
                    source.slide_id, channel.id
                ))
            })?,
        );
    }
    let table = MarkTable::new(
        ids,
        columns,
        recipe.limits.maximum_rows_per_slide,
        MAXIMUM_RECIPE_BYTES,
    )
    .map_err(|error| invalid(format!("slide {}: {error}", source.slide_id)))?;
    let panel_artifact = table
        .declared_artifact_ref(
            &SlideId::new(&source.slide_id).map_err(|error| invalid(error.to_string()))?,
            &frame,
        )
        .map_err(|error| invalid(error.to_string()))?;
    let (x, y) = source
        .coordinates_um
        .iter()
        .map(|point| (point[0], point[1]))
        .unzip();
    Ok(PreparedSlide {
        source,
        source_artifact,
        panel_artifact,
        table,
        x,
        y,
        frame,
        window,
    })
}

fn declaration(channel: &Channel) -> Result<AssayMarkDeclaration> {
    if !label(&channel.provenance) {
        return Err(invalid(
            "every channel requires bounded assay/processing provenance",
        ));
    }
    let status = match channel.measurement_status.as_str() {
        "measured" => MeasurementStatus::Measured,
        "imported_prediction" => MeasurementStatus::ImportedPrediction,
        "morphology_prediction" => MeasurementStatus::MorphologyPrediction,
        _ => return Err(invalid("channel measurement_status must be measured, imported_prediction or morphology_prediction")),
    };
    let provenance =
        ArtifactId::from_str(&ContentDigest::from_bytes(channel.provenance.as_bytes()).to_string())
            .map_err(|error| invalid(error.to_string()))?;
    AssayMarkDeclaration::new(
        ScalarMarkId::new(&channel.id).map_err(|error| invalid(error.to_string()))?,
        &channel.label,
        &channel.unit,
        status,
        provenance,
    )
    .map_err(|error| invalid(error.to_string()))
}

fn label(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && value.trim() == value
        && !value.chars().any(char::is_control)
}
