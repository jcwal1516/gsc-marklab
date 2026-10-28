use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::{
    invalid, label, resource, SpaceError, SpaceVariable, SpaceVariableKind, SPACE_FORMAT_VERSION,
};
use crate::geom::spatial_index::SpatialIndex2D;
#[cfg(feature = "cli")]
use crate::multimodal::load_cellvit_he_cell_table_csv;
use crate::multiplex_study::model::{Channel, ChannelKind, Recipe};
use crate::{ObservationWindow2D, ObservationWindowLimits};

pub const SPACE_REQUEST_FORMAT: &str = "marklab.space_request";
const MAXIMUM_REQUEST_BYTES: u64 = 256 * 1024 * 1024;
const MAXIMUM_RECIPE_BYTES: u64 = 16 * 1024 * 1024;
#[cfg(feature = "cli")]
const MAXIMUM_CELL_TABLE_BYTES: u64 = 1024 * 1024 * 1024;
#[cfg(feature = "cli")]
const MAXIMUM_WINDOW_BYTES: u64 = 16 * 1024 * 1024;
const MAXIMUM_RADII: usize = 64;
/// Class index of a cell with no class in an object channel (SPACE's background).
pub(crate) const NO_CLASS: u32 = u32::MAX;
/// Largest scalar magnitude accepted. Binning squares differences of disc means, so
/// values beyond about 1e154 would overflow to infinity; this bound keeps every census mean,
/// grid step and squared distance finite.
const MAXIMUM_SCALAR_MAGNITUDE: f64 = 1e150;
#[cfg(feature = "cli")]
const CELLVIT_CLASS_CHANNEL: &str = "cellvit_class";
#[cfg(feature = "cli")]
const CELLVIT_PROBABILITY_CHANNEL: &str = "class_probability";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    format: String,
    version: u32,
    source: Source,
    #[serde(default)]
    objects: Vec<String>,
    #[serde(default)]
    linked_scalars: Vec<LinkedScalarDeclaration>,
    #[serde(default)]
    independent_scalars: Vec<String>,
    #[serde(default)]
    merge: Vec<MergeRule>,
    census: SpaceCensusDesign,
    #[serde(default)]
    limits: SpaceLimits,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Source {
    Inline {
        channels: Vec<Channel>,
        specimens: Vec<InlineSpecimen>,
    },
    MultiplexStudyRecipe {
        path: PathBuf,
    },
    CellvitHeCsv {
        specimens: Vec<CellvitSpecimen>,
        #[serde(default)]
        minimum_class_probability: f64,
    },
}

/// One CellViT classification table and the specimen it describes.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(
    not(feature = "cli"),
    expect(
        dead_code,
        reason = "parsed so the request format is the same; read only with cli"
    )
)]
struct CellvitSpecimen {
    path: PathBuf,
    window_path: PathBuf,
    specimen_id: String,
    coordinate_frame_id: String,
    #[serde(default)]
    groups: BTreeMap<String, String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InlineSpecimen {
    specimen_id: String,
    #[serde(default)]
    groups: BTreeMap<String, String>,
    coordinate_frame_id: String,
    window: serde_json::Value,
    cell_ids: Vec<String>,
    coordinates_um: Vec<[f64; 2]>,
    observations: BTreeMap<String, Vec<Option<f64>>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LinkedScalarDeclaration {
    channel: String,
    class: String,
    scalar: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MergeRule {
    channel: String,
    classes: Vec<String>,
    into: String,
}

/// Where and how often discs are drawn; shared by every specimen.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SpaceCensusDesign {
    /// Strictly increasing disc radii in micrometres.
    pub radii_um: Vec<f64>,
    /// Discs drawn without replacement at each radius (SPACE `sample_size`). Needed to draw
    /// discs for specimens without `seed_cell_ids`; `space suggest` reads a request without it.
    #[serde(default)]
    pub sample_size: Option<Vec<usize>>,
    /// Object classes whose cells may be disc centres, by channel (SPACE `seed_points`).
    /// Absent: every cell with a class in any object channel. Channels not listed contribute none.
    #[serde(default)]
    pub seed_classes: Option<BTreeMap<String, Vec<String>>>,
    /// Explicit disc centres by specimen, one list of cell IDs per radius.
    #[serde(default)]
    pub seed_cell_ids: Option<BTreeMap<String, Vec<Vec<String>>>>,
    /// Base seed for disc sampling and randomized censuses.
    pub seed: u64,
}

/// Declared ceilings; each must be positive and within its fixed hard maximum.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct SpaceLimits {
    pub maximum_cells_per_specimen: usize,
    pub maximum_specimens: usize,
    pub maximum_variables: usize,
    /// Member rows summed over the discs of one specimen at one radius.
    pub maximum_disc_members: usize,
    pub maximum_ensembles: usize,
    pub maximum_bootstraps: usize,
    /// Ensembles × (bootstraps + 1) × discs, the cell visits of every joint-entropy count.
    pub maximum_entropy_visits: u64,
    pub maximum_retained_bytes: usize,
}

impl Default for SpaceLimits {
    fn default() -> Self {
        Self {
            maximum_cells_per_specimen: 1_000_000,
            maximum_specimens: 256,
            maximum_variables: 128,
            maximum_disc_members: 100_000_000,
            maximum_ensembles: 1_000_000,
            maximum_bootstraps: 10_000,
            maximum_entropy_visits: 50_000_000_000,
            maximum_retained_bytes: 4 * 1024 * 1024 * 1024,
        }
    }
}

const HARD_LIMITS: SpaceLimits = SpaceLimits {
    maximum_cells_per_specimen: 10_000_000,
    maximum_specimens: 4096,
    maximum_variables: 1024,
    maximum_disc_members: 1_000_000_000,
    maximum_ensembles: 50_000_000,
    maximum_bootstraps: 1_000_000,
    maximum_entropy_visits: 10_000_000_000_000,
    maximum_retained_bytes: 64 * 1024 * 1024 * 1024,
};

/// Validated specimens, census variables and census design.
pub struct SpaceInput {
    pub(crate) source_kind: &'static str,
    pub(crate) object_channels: Vec<ObjectChannel>,
    pub(crate) variables: Vec<VariableSpec>,
    pub(crate) variable_descriptions: Vec<SpaceVariable>,
    pub(crate) census: SpaceCensusDesign,
    pub(crate) limits: SpaceLimits,
    /// Sorted by specimen ID; the position fixes each specimen's random streams.
    pub(crate) specimens: Vec<Specimen>,
}

impl SpaceInput {
    /// Specimen IDs in canonical order.
    pub fn specimen_ids(&self) -> impl Iterator<Item = &str> {
        self.specimens.iter().map(|specimen| specimen.id.as_str())
    }

    /// Every declared census variable, before restricting to classes a specimen contains.
    pub fn variables(&self) -> &[SpaceVariable] {
        &self.variable_descriptions
    }

    pub(crate) fn variable_index(&self, name: &str) -> Option<usize> {
        self.variable_descriptions
            .iter()
            .position(|variable| variable.name == name)
    }
}

pub(crate) struct ObjectChannel {
    pub id: String,
    pub classes: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum VariableSpec {
    Object {
        map: usize,
        class: u32,
    },
    Linked {
        map: usize,
        class: u32,
        scalar: usize,
    },
    Independent {
        scalar: usize,
    },
}

pub(crate) struct Specimen {
    pub id: String,
    pub groups: BTreeMap<String, String>,
    pub cell_ids: Vec<String>,
    pub x_um: Vec<f64>,
    pub y_um: Vec<f64>,
    /// Per object channel, the class of every cell or [`NO_CLASS`].
    pub classes: Vec<Vec<u32>>,
    /// Per object channel, the number of cells of each class.
    pub class_counts: Vec<Vec<usize>>,
    /// Per scalar channel, the value of every cell.
    pub scalars: Vec<Vec<f64>>,
    pub index: SpatialIndex2D,
    /// Cell indices of explicit disc centres, one list per radius.
    pub explicit_seeds: Option<Vec<Vec<usize>>>,
}

impl Specimen {
    /// Whether the specimen contains the cells a variable describes.
    pub(crate) fn has(&self, variable: VariableSpec) -> bool {
        match variable {
            VariableSpec::Object { map, class } | VariableSpec::Linked { map, class, .. } => {
                self.class_counts[map][class as usize] > 0
            }
            VariableSpec::Independent { .. } => true,
        }
    }
}

struct RawSpecimen {
    id: String,
    groups: BTreeMap<String, String>,
    coordinate_frame_id: String,
    window_geojson: String,
    cell_ids: Vec<String>,
    coordinates_um: Vec<[f64; 2]>,
    observations: BTreeMap<String, Vec<Option<f64>>>,
}

/// Read a SPACE request file; file sources resolve relative to its directory.
pub fn load_space_input(path: &Path) -> Result<SpaceInput, SpaceError> {
    let bytes = read_bounded(path, MAXIMUM_REQUEST_BYTES, "request")?;
    let base = path.parent().unwrap_or_else(|| Path::new("."));
    prepare(&bytes, Some(base))
}

/// Decode a SPACE request whose specimens are inline; file sources need [`load_space_input`].
pub fn space_input_from_json(bytes: &[u8]) -> Result<SpaceInput, SpaceError> {
    if bytes.len() as u64 > MAXIMUM_REQUEST_BYTES {
        return Err(resource("SPACE request exceeds 256 MiB"));
    }
    prepare(bytes, None)
}

fn prepare(bytes: &[u8], base: Option<&Path>) -> Result<SpaceInput, SpaceError> {
    let request: Request = serde_json::from_slice(bytes)
        .map_err(|error| invalid(format!("invalid SPACE request JSON: {error}")))?;
    if request.format != SPACE_REQUEST_FORMAT || request.version != SPACE_FORMAT_VERSION {
        return Err(invalid(format!(
            "format and version must be {SPACE_REQUEST_FORMAT} version {SPACE_FORMAT_VERSION}"
        )));
    }
    validate_limits(request.limits)?;
    let (source_kind, channels, raw) =
        load_source(request.source, base, request.limits.maximum_specimens)?;
    let channels = declared_channels(channels)?;

    let mut seen_objects = BTreeSet::new();
    let mut object_channels = Vec::with_capacity(request.objects.len());
    let mut class_maps = Vec::with_capacity(request.objects.len());
    for id in &request.objects {
        if !seen_objects.insert(id) {
            return Err(invalid(format!("object channel {id} is listed twice")));
        }
        let channel = channels
            .get(id.as_str())
            .ok_or_else(|| invalid(format!("object channel {id} is not declared")))?;
        if channel.kind != ChannelKind::Categorical {
            return Err(invalid(format!("object channel {id} must be categorical")));
        }
        let (classes, map) = merged_classes(channel, &request.merge)?;
        object_channels.push(ObjectChannel {
            id: id.clone(),
            classes,
        });
        class_maps.push(map);
    }
    for rule in &request.merge {
        if !seen_objects.contains(&rule.channel) {
            return Err(invalid(format!(
                "merge names {} which is not an object channel",
                rule.channel
            )));
        }
    }

    let mut scalar_channels = Vec::<String>::new();
    let mut scalar_index = |id: &str| -> Result<usize, SpaceError> {
        if let Some(position) = scalar_channels.iter().position(|known| known == id) {
            return Ok(position);
        }
        let channel = channels
            .get(id)
            .ok_or_else(|| invalid(format!("scalar channel {id} is not declared")))?;
        if channel.kind == ChannelKind::Categorical {
            return Err(invalid(format!(
                "scalar channel {id} must be continuous or binary"
            )));
        }
        scalar_channels.push(id.to_owned());
        Ok(scalar_channels.len() - 1)
    };

    let mut variables = Vec::new();
    let mut descriptions = Vec::new();
    for (map, channel) in object_channels.iter().enumerate() {
        for (class, name) in channel.classes.iter().enumerate() {
            variables.push(VariableSpec::Object {
                map,
                class: class as u32,
            });
            descriptions.push(SpaceVariable {
                name: format!("{}.{name}", channel.id),
                kind: SpaceVariableKind::Object,
                channel: Some(channel.id.clone()),
                class: Some(name.clone()),
                scalar: None,
            });
        }
    }
    for link in &request.linked_scalars {
        let map = object_channels
            .iter()
            .position(|channel| channel.id == link.channel)
            .ok_or_else(|| {
                invalid(format!(
                    "linked scalar names {} which is not an object channel",
                    link.channel
                ))
            })?;
        let class = class_position(&object_channels[map], &link.class)?;
        let scalar = scalar_index(&link.scalar)?;
        variables.push(VariableSpec::Linked { map, class, scalar });
        descriptions.push(SpaceVariable {
            name: format!("{}.{}_{}", link.channel, link.class, link.scalar),
            kind: SpaceVariableKind::LinkedScalar,
            channel: Some(link.channel.clone()),
            class: Some(link.class.clone()),
            scalar: Some(link.scalar.clone()),
        });
    }
    for id in &request.independent_scalars {
        let scalar = scalar_index(id)?;
        variables.push(VariableSpec::Independent { scalar });
        descriptions.push(SpaceVariable {
            name: id.clone(),
            kind: SpaceVariableKind::IndependentScalar,
            channel: None,
            class: None,
            scalar: Some(id.clone()),
        });
    }
    if variables.is_empty() {
        return Err(invalid(
            "declare at least one object channel, linked scalar or independent scalar",
        ));
    }
    if variables.len() > request.limits.maximum_variables {
        return Err(resource(format!(
            "{} census variables exceed the maximum of {}",
            variables.len(),
            request.limits.maximum_variables
        )));
    }
    let mut names = BTreeSet::new();
    for description in &descriptions {
        if !names.insert(description.name.as_str()) {
            return Err(invalid(format!(
                "census variable name {} is ambiguous; rename a channel or class",
                description.name
            )));
        }
    }

    let census = request.census;
    validate_census(&census, &object_channels)?;
    if raw.is_empty() || raw.len() > request.limits.maximum_specimens {
        return Err(resource(format!(
            "{} specimens; between 1 and {} are admitted",
            raw.len(),
            request.limits.maximum_specimens
        )));
    }
    let mut specimens = raw
        .into_iter()
        .map(|raw| {
            prepare_specimen(
                raw,
                &channels,
                &object_channels,
                &class_maps,
                &scalar_channels,
                request.limits,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    specimens.sort_by(|left, right| left.id.cmp(&right.id));
    if specimens.windows(2).any(|pair| pair[0].id == pair[1].id) {
        return Err(invalid("specimen IDs must be unique"));
    }
    attach_explicit_seeds(&mut specimens, &census)?;

    Ok(SpaceInput {
        source_kind,
        object_channels,
        variables,
        variable_descriptions: descriptions,
        census,
        limits: request.limits,
        specimens,
    })
}

fn load_source(
    source: Source,
    base: Option<&Path>,
    maximum_specimens: usize,
) -> Result<(&'static str, Vec<Channel>, Vec<RawSpecimen>), SpaceError> {
    match source {
        Source::Inline {
            channels,
            specimens,
        } => {
            let raw = specimens
                .into_iter()
                .map(|specimen| {
                    let window_geojson = serde_json::to_string(&specimen.window)
                        .map_err(|error| invalid(format!("window JSON: {error}")))?;
                    Ok(RawSpecimen {
                        id: specimen.specimen_id,
                        groups: specimen.groups,
                        coordinate_frame_id: specimen.coordinate_frame_id,
                        window_geojson,
                        cell_ids: specimen.cell_ids,
                        coordinates_um: specimen.coordinates_um,
                        observations: specimen.observations,
                    })
                })
                .collect::<Result<Vec<_>, SpaceError>>()?;
            Ok(("inline", channels, raw))
        }
        Source::MultiplexStudyRecipe { path } => {
            let path = resolve(base, &path)?;
            let bytes = read_bounded(&path, MAXIMUM_RECIPE_BYTES, "multiplex study recipe")?;
            let recipe: Recipe = serde_json::from_slice(&bytes)
                .map_err(|error| invalid(format!("invalid multiplex study recipe: {error}")))?;
            if recipe.format != "marklab.multiplex_study_recipe" || recipe.version != 1 {
                return Err(invalid(
                    "the recipe must be marklab.multiplex_study_recipe version 1",
                ));
            }
            let raw = recipe
                .slides
                .into_iter()
                .map(|slide| {
                    let window_geojson = serde_json::to_string(&slide.window)
                        .map_err(|error| invalid(format!("window JSON: {error}")))?;
                    Ok(RawSpecimen {
                        id: slide.slide_id,
                        groups: BTreeMap::from([("group".to_owned(), slide.group)]),
                        coordinate_frame_id: slide.coordinate_frame_id,
                        window_geojson,
                        cell_ids: slide.cell_ids,
                        coordinates_um: slide.coordinates_um,
                        observations: slide.observations,
                    })
                })
                .collect::<Result<Vec<_>, SpaceError>>()?;
            Ok(("multiplex_study_recipe", recipe.channels, raw))
        }
        Source::CellvitHeCsv {
            specimens,
            minimum_class_probability,
        } => {
            #[cfg(feature = "cli")]
            {
                cellvit_source(
                    specimens,
                    minimum_class_probability,
                    base,
                    maximum_specimens,
                )
            }
            #[cfg(not(feature = "cli"))]
            {
                let _ = (specimens, minimum_class_probability, maximum_specimens);
                Err(invalid(
                    "the cellvit_he_csv source reads CSV and needs Marklab's cli feature",
                ))
            }
        }
    }
}

/// CellViT H&E classification tables, one per specimen, with one class list across them.
#[cfg(feature = "cli")]
fn cellvit_source(
    specimens: Vec<CellvitSpecimen>,
    minimum_class_probability: f64,
    base: Option<&Path>,
    maximum_specimens: usize,
) -> Result<(&'static str, Vec<Channel>, Vec<RawSpecimen>), SpaceError> {
    // Checked before any table is read; other sources are bounded by their file size.
    if specimens.len() > maximum_specimens {
        return Err(resource(format!(
            "{} CellViT tables exceed the maximum of {maximum_specimens} specimens",
            specimens.len()
        )));
    }
    let mut tables = Vec::with_capacity(specimens.len());
    for specimen in specimens {
        let path = resolve(base, &specimen.path)?;
        check_regular_file(&path, MAXIMUM_CELL_TABLE_BYTES, "CellViT cell table")?;
        let cells = load_cellvit_he_cell_table_csv(&path, minimum_class_probability)
            .map_err(|error| invalid(format!("{}: {error}", specimen.specimen_id)))?;
        let window_path = resolve(base, &specimen.window_path)?;
        let window = read_bounded(&window_path, MAXIMUM_WINDOW_BYTES, "window")?;
        let window_geojson =
            String::from_utf8(window).map_err(|_| invalid("window GeoJSON must be UTF-8"))?;
        tables.push((specimen, cells, window_geojson));
    }
    // One class list across every table, so a class means the same in each specimen.
    let classes = tables
        .iter()
        .flat_map(|(_, cells, _)| cells.iter().filter_map(|cell| cell.cell_type.clone()))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let provenance = "CellViT H&E cell table (cell_id, x_centroid_um, y_centroid_um, predicted_class, class_probability)";
    let channels = vec![
        Channel {
            id: CELLVIT_CLASS_CHANNEL.into(),
            label: "CellViT predicted class".into(),
            kind: ChannelKind::Categorical,
            unit: "categorical".into(),
            measurement_status: "imported_prediction".into(),
            provenance: provenance.into(),
            levels: classes.clone(),
        },
        Channel {
            id: CELLVIT_PROBABILITY_CHANNEL.into(),
            label: "CellViT predicted-class probability".into(),
            kind: ChannelKind::Continuous,
            unit: "probability".into(),
            measurement_status: "imported_prediction".into(),
            provenance: provenance.into(),
            levels: Vec::new(),
        },
    ];
    let raw = tables
        .into_iter()
        .map(|(specimen, cells, window_geojson)| {
            let mut class_values = Vec::with_capacity(cells.len());
            let mut probabilities = Vec::with_capacity(cells.len());
            let mut cell_ids = Vec::with_capacity(cells.len());
            let mut coordinates_um = Vec::with_capacity(cells.len());
            for cell in cells {
                class_values.push(cell.cell_type.as_ref().map(|class| {
                    classes
                        .binary_search(class)
                        .expect("classes collected from the same cells") as f64
                }));
                probabilities.push(cell.cell_type_probability);
                cell_ids.push(cell.cell_id);
                coordinates_um.push([cell.x_um, cell.y_um]);
            }
            RawSpecimen {
                id: specimen.specimen_id,
                groups: specimen.groups,
                coordinate_frame_id: specimen.coordinate_frame_id,
                window_geojson,
                cell_ids,
                coordinates_um,
                observations: BTreeMap::from([
                    (CELLVIT_CLASS_CHANNEL.to_owned(), class_values),
                    (CELLVIT_PROBABILITY_CHANNEL.to_owned(), probabilities),
                ]),
            }
        })
        .collect();
    Ok(("cellvit_he_csv", channels, raw))
}

fn resolve(base: Option<&Path>, path: &Path) -> Result<PathBuf, SpaceError> {
    match base {
        Some(base) => Ok(base.join(path)),
        None => Err(invalid(
            "file sources resolve relative to a request file; load the request from its path",
        )),
    }
}

fn check_regular_file(path: &Path, maximum: u64, what: &str) -> Result<(), SpaceError> {
    let metadata = fs::metadata(path)
        .map_err(|error| SpaceError::Io(format!("{what} {}: {error}", path.display())))?;
    if !metadata.is_file() || metadata.len() > maximum {
        return Err(SpaceError::Io(format!(
            "{what} {} must be a regular file within {maximum} bytes",
            path.display()
        )));
    }
    Ok(())
}

pub(super) fn read_bounded(path: &Path, maximum: u64, what: &str) -> Result<Vec<u8>, SpaceError> {
    check_regular_file(path, maximum, what)?;
    let mut bytes = Vec::new();
    fs::File::open(path)
        .and_then(|file| file.take(maximum + 1).read_to_end(&mut bytes))
        .map_err(|error| SpaceError::Io(format!("{what} {}: {error}", path.display())))?;
    if bytes.len() as u64 > maximum {
        return Err(SpaceError::Io(format!(
            "{what} grew beyond {maximum} bytes"
        )));
    }
    Ok(bytes)
}

fn validate_limits(limits: SpaceLimits) -> Result<(), SpaceError> {
    let within = |value: u64, maximum: u64| (1..=maximum).contains(&value);
    let ok = within(
        limits.maximum_cells_per_specimen as u64,
        HARD_LIMITS.maximum_cells_per_specimen as u64,
    ) && within(
        limits.maximum_specimens as u64,
        HARD_LIMITS.maximum_specimens as u64,
    ) && within(
        limits.maximum_variables as u64,
        HARD_LIMITS.maximum_variables as u64,
    ) && within(
        limits.maximum_disc_members as u64,
        HARD_LIMITS.maximum_disc_members as u64,
    ) && within(
        limits.maximum_ensembles as u64,
        HARD_LIMITS.maximum_ensembles as u64,
    ) && within(
        limits.maximum_bootstraps as u64,
        HARD_LIMITS.maximum_bootstraps as u64,
    ) && within(
        limits.maximum_entropy_visits,
        HARD_LIMITS.maximum_entropy_visits,
    ) && within(
        limits.maximum_retained_bytes as u64,
        HARD_LIMITS.maximum_retained_bytes as u64,
    );
    if ok {
        Ok(())
    } else {
        Err(resource(format!(
            "every limit must be positive and within the hard maxima {HARD_LIMITS:?}"
        )))
    }
}

fn declared_channels(channels: Vec<Channel>) -> Result<BTreeMap<String, Channel>, SpaceError> {
    let mut declared = BTreeMap::new();
    for channel in channels {
        if !label(&channel.id) {
            return Err(invalid("channel IDs must be nonempty bounded labels"));
        }
        if channel.kind == ChannelKind::Categorical {
            if channel.levels.is_empty() {
                return Err(invalid(format!(
                    "categorical channel {} declares no classes",
                    channel.id
                )));
            }
            let mut levels = BTreeSet::new();
            for level in &channel.levels {
                if !label(level) || !levels.insert(level) {
                    return Err(invalid(format!(
                        "categorical channel {} has an empty, unbounded or duplicated class",
                        channel.id
                    )));
                }
            }
        } else if !channel.levels.is_empty() {
            return Err(invalid(format!(
                "only categorical channels declare classes; {} is not categorical",
                channel.id
            )));
        }
        if let Some(previous) = declared.insert(channel.id.clone(), channel) {
            return Err(invalid(format!(
                "channel {} is declared twice",
                previous.id
            )));
        }
    }
    Ok(declared)
}

/// Apply merge rules to one channel's classes: the merged class takes the position of the
/// first class it absorbs. Returns the new classes and the map from old to new positions.
fn merged_classes(
    channel: &Channel,
    rules: &[MergeRule],
) -> Result<(Vec<String>, Vec<u32>), SpaceError> {
    let mut target = vec![None::<usize>; channel.levels.len()];
    let rules = rules
        .iter()
        .filter(|rule| rule.channel == channel.id)
        .collect::<Vec<_>>();
    for (rule_index, rule) in rules.iter().enumerate() {
        if rule.classes.len() < 2 || !label(&rule.into) {
            return Err(invalid(format!(
                "a merge on {} needs at least two classes and a bounded new name",
                channel.id
            )));
        }
        for class in &rule.classes {
            let position = channel
                .levels
                .iter()
                .position(|level| level == class)
                .ok_or_else(|| {
                    invalid(format!(
                        "merge class {class} is not a class of {}",
                        channel.id
                    ))
                })?;
            if target[position].replace(rule_index).is_some() {
                return Err(invalid(format!(
                    "class {class} of {} is merged more than once",
                    channel.id
                )));
            }
        }
    }
    let mut classes = Vec::<String>::new();
    let mut emitted = vec![None::<u32>; rules.len()];
    let mut map = Vec::with_capacity(channel.levels.len());
    for (position, level) in channel.levels.iter().enumerate() {
        let index = match target[position] {
            Some(rule) => *emitted[rule].get_or_insert_with(|| {
                classes.push(rules[rule].into.clone());
                (classes.len() - 1) as u32
            }),
            None => {
                classes.push(level.clone());
                (classes.len() - 1) as u32
            }
        };
        map.push(index);
    }
    let unique = classes.iter().collect::<BTreeSet<_>>();
    if unique.len() != classes.len() {
        return Err(invalid(format!(
            "merged class names on {} collide with remaining classes",
            channel.id
        )));
    }
    Ok((classes, map))
}

fn class_position(channel: &ObjectChannel, class: &str) -> Result<u32, SpaceError> {
    channel
        .classes
        .iter()
        .position(|known| known == class)
        .map(|position| position as u32)
        .ok_or_else(|| invalid(format!("{class} is not a class of {}", channel.id)))
}

fn validate_census(
    census: &SpaceCensusDesign,
    object_channels: &[ObjectChannel],
) -> Result<(), SpaceError> {
    let radii = &census.radii_um;
    if radii.is_empty() || radii.len() > MAXIMUM_RADII {
        return Err(invalid(format!("declare 1 to {MAXIMUM_RADII} radii")));
    }
    if radii
        .iter()
        .any(|radius| !radius.is_finite() || *radius <= 0.0)
        || radii.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return Err(invalid(
            "radii must be finite, positive and strictly increasing",
        ));
    }
    if let Some(sizes) = &census.sample_size {
        if sizes.len() != radii.len() || sizes.contains(&0) {
            return Err(invalid(
                "sample_size needs one positive disc count per radius",
            ));
        }
    }
    if let Some(seed_classes) = &census.seed_classes {
        for (channel, classes) in seed_classes {
            let channel = object_channels
                .iter()
                .find(|known| &known.id == channel)
                .ok_or_else(|| {
                    invalid(format!(
                        "seed_classes names {channel} which is not an object channel"
                    ))
                })?;
            for class in classes {
                class_position(channel, class)?;
            }
        }
    }
    Ok(())
}

fn prepare_specimen(
    raw: RawSpecimen,
    channels: &BTreeMap<String, Channel>,
    object_channels: &[ObjectChannel],
    class_maps: &[Vec<u32>],
    scalar_channels: &[String],
    limits: SpaceLimits,
) -> Result<Specimen, SpaceError> {
    let id = raw.id;
    if !label(&id) || !label(&raw.coordinate_frame_id) {
        return Err(invalid(
            "specimen and coordinate-frame IDs must be nonempty bounded labels",
        ));
    }
    if raw
        .groups
        .iter()
        .any(|(factor, level)| !label(factor) || !label(level))
    {
        return Err(invalid(format!(
            "specimen {id} has an empty or unbounded grouping factor"
        )));
    }
    let rows = raw.cell_ids.len();
    if rows == 0 || rows > limits.maximum_cells_per_specimen {
        return Err(resource(format!(
            "specimen {id} has {rows} cells; between 1 and {} are admitted",
            limits.maximum_cells_per_specimen
        )));
    }
    if raw.coordinates_um.len() != rows {
        return Err(invalid(format!(
            "specimen {id} has {rows} cell IDs but {} coordinates",
            raw.coordinates_um.len()
        )));
    }
    let mut seen = BTreeSet::new();
    for cell in &raw.cell_ids {
        if !label(cell) || !seen.insert(cell.as_str()) {
            return Err(invalid(format!(
                "specimen {id} has an empty, unbounded or duplicated cell ID"
            )));
        }
    }
    drop(seen);
    let window = ObservationWindow2D::from_geojson_str(
        &raw.window_geojson,
        ObservationWindowLimits::default(),
    )
    .map_err(|error| invalid(format!("specimen {id} window: {error}")))?;
    let mut x_um = Vec::with_capacity(rows);
    let mut y_um = Vec::with_capacity(rows);
    for (row, [x, y]) in raw.coordinates_um.iter().copied().enumerate() {
        if !x.is_finite() || !y.is_finite() {
            return Err(invalid(format!(
                "specimen {id} cell row {row} has non-finite coordinates"
            )));
        }
        if !window.contains(x, y) {
            return Err(invalid(format!(
                "specimen {id} cell {} lies outside the observation window",
                raw.cell_ids[row]
            )));
        }
        x_um.push(x);
        y_um.push(y);
    }
    drop(window);
    for key in raw.observations.keys() {
        if !channels.contains_key(key) {
            return Err(invalid(format!(
                "specimen {id} observes undeclared channel {key}"
            )));
        }
    }
    let column = |channel: &str| -> Result<&Vec<Option<f64>>, SpaceError> {
        let values = raw
            .observations
            .get(channel)
            .ok_or_else(|| invalid(format!("specimen {id} does not observe {channel}")))?;
        if values.len() != rows {
            return Err(invalid(format!(
                "specimen {id} has {} values of {channel} for {rows} cells",
                values.len()
            )));
        }
        Ok(values)
    };

    let mut classes = Vec::with_capacity(object_channels.len());
    let mut class_counts = Vec::with_capacity(object_channels.len());
    for (channel, map) in object_channels.iter().zip(class_maps) {
        let mut counts = vec![0_usize; channel.classes.len()];
        let values = column(&channel.id)?
            .iter()
            .enumerate()
            .map(|(row, value)| match value {
                None => Ok(NO_CLASS),
                Some(code)
                    if code.fract() == 0.0 && *code >= 0.0 && (*code as usize) < map.len() =>
                {
                    let class = map[*code as usize];
                    counts[class as usize] += 1;
                    Ok(class)
                }
                Some(_) => Err(invalid(format!(
                    "specimen {id} cell row {row} has an invalid class code for {}",
                    channel.id
                ))),
            })
            .collect::<Result<Vec<_>, _>>()?;
        classes.push(values);
        class_counts.push(counts);
    }
    let mut scalars = Vec::with_capacity(scalar_channels.len());
    for channel in scalar_channels {
        let binary = channels[channel].kind == ChannelKind::Binary;
        let values = column(channel)?
            .iter()
            .enumerate()
            .map(|(row, value)| match value {
                Some(value)
                    if value.abs() <= MAXIMUM_SCALAR_MAGNITUDE
                        && (!binary || *value == 0.0 || *value == 1.0) =>
                {
                    Ok(*value)
                }
                Some(_) => Err(invalid(format!(
                    "specimen {id} cell row {row} has an invalid value of {channel}; scalars must be finite with magnitude at most {MAXIMUM_SCALAR_MAGNITUDE:e}"
                ))),
                None => Err(invalid(format!(
                    "specimen {id} cell row {row} is missing {channel}; SPACE scalars must be observed on every cell"
                ))),
            })
            .collect::<Result<Vec<_>, _>>()?;
        scalars.push(values);
    }
    let index = SpatialIndex2D::new(&x_um, &y_um)
        .map_err(|error| invalid(format!("specimen {id} spatial index: {error}")))?;
    Ok(Specimen {
        id,
        groups: raw.groups,
        cell_ids: raw.cell_ids,
        x_um,
        y_um,
        classes,
        class_counts,
        scalars,
        index,
        explicit_seeds: None,
    })
}

fn attach_explicit_seeds(
    specimens: &mut [Specimen],
    census: &SpaceCensusDesign,
) -> Result<(), SpaceError> {
    if let Some(explicit) = &census.seed_cell_ids {
        for (specimen_id, lists) in explicit {
            let specimen = specimens
                .iter_mut()
                .find(|specimen| &specimen.id == specimen_id)
                .ok_or_else(|| {
                    invalid(format!(
                        "seed_cell_ids names unknown specimen {specimen_id}"
                    ))
                })?;
            if lists.len() != census.radii_um.len() {
                return Err(invalid(format!(
                    "seed_cell_ids for {specimen_id} needs one list per radius"
                )));
            }
            let positions = specimen
                .cell_ids
                .iter()
                .enumerate()
                .map(|(row, cell)| (cell.as_str(), row))
                .collect::<BTreeMap<_, _>>();
            let mut resolved = Vec::with_capacity(lists.len());
            for list in lists {
                let mut unique = BTreeSet::new();
                let rows = list
                    .iter()
                    .map(|cell| {
                        let row = *positions.get(cell.as_str()).ok_or_else(|| {
                            invalid(format!("seed cell {cell} is not in specimen {specimen_id}"))
                        })?;
                        if !unique.insert(row) {
                            return Err(invalid(format!(
                                "seed cell {cell} repeats at one radius of {specimen_id}"
                            )));
                        }
                        Ok(row)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                if rows.is_empty() {
                    return Err(invalid(format!(
                        "seed_cell_ids for {specimen_id} has an empty radius"
                    )));
                }
                resolved.push(rows);
            }
            specimen.explicit_seeds = Some(resolved);
        }
    }
    Ok(())
}
