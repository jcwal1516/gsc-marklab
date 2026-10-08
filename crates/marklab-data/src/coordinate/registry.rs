use std::collections::HashMap;

use marklab_core::{CoordinateFrameId, SectionId, TransformId, UncertaintyId};

use super::{
    Coordinate2D, Coordinate3D, CoordinateError, CoordinateFrame, CoordinateSpace, FrameTransform,
    SerialSection, SerialSectionSeries, SpatialAxis, SpatialDimension, UncertaintyReference,
};

/// A validated acyclic registry of explicit coordinate frames and transforms.
#[derive(Clone, Debug)]
pub struct CoordinateRegistry {
    frames: Vec<CoordinateFrame>,
    frame_indices: HashMap<CoordinateFrameId, usize>,
    uncertainties: Vec<UncertaintyReference>,
    uncertainty_indices: HashMap<UncertaintyId, usize>,
    transforms: Vec<FrameTransform>,
    transform_indices: HashMap<TransformId, usize>,
    serial_section_series: Vec<SerialSectionSeries>,
    serial_series_indices: HashMap<CoordinateFrameId, usize>,
    serial_section_indices: HashMap<SectionId, (usize, usize)>,
}

struct IndexedSerialSections {
    series: HashMap<CoordinateFrameId, usize>,
    sections: HashMap<SectionId, (usize, usize)>,
}

impl CoordinateRegistry {
    /// Validate declarations in their supplied order.
    pub fn new(
        frames: Vec<CoordinateFrame>,
        uncertainties: Vec<UncertaintyReference>,
        transforms: Vec<FrameTransform>,
        serial_section_series: Vec<SerialSectionSeries>,
    ) -> Result<Self, CoordinateError> {
        let frame_indices = index_frames(&frames)?;
        let uncertainty_indices = index_uncertainties(&uncertainties)?;
        let transform_indices = index_transforms(&transforms)?;
        validate_uncertainty_frames(&uncertainties, &frame_indices)?;
        let endpoints = validate_transforms(
            &transforms,
            &frames,
            &frame_indices,
            &uncertainties,
            &uncertainty_indices,
        )?;
        reject_transform_cycles(&frames, &transforms, &endpoints)?;
        let indexed_serial_sections = index_serial_sections(
            &serial_section_series,
            &frames,
            &frame_indices,
            &uncertainties,
            &uncertainty_indices,
        )?;

        Ok(Self {
            frames,
            frame_indices,
            uncertainties,
            uncertainty_indices,
            transforms,
            transform_indices,
            serial_section_series,
            serial_series_indices: indexed_serial_sections.series,
            serial_section_indices: indexed_serial_sections.sections,
        })
    }

    /// Registered frames in declaration order.
    pub fn frames(&self) -> &[CoordinateFrame] {
        &self.frames
    }

    /// Registered uncertainty references in declaration order.
    pub fn uncertainties(&self) -> &[UncertaintyReference] {
        &self.uncertainties
    }

    /// Registered transforms in declaration order.
    pub fn transforms(&self) -> &[FrameTransform] {
        &self.transforms
    }

    /// Registered serial-section series in declaration order.
    pub fn serial_section_series(&self) -> &[SerialSectionSeries] {
        &self.serial_section_series
    }

    /// Look up a frame by its exact identity.
    pub fn frame(&self, id: &CoordinateFrameId) -> Option<&CoordinateFrame> {
        self.frame_indices.get(id).map(|index| &self.frames[*index])
    }

    /// Look up an uncertainty reference by its exact identity.
    pub fn uncertainty(&self, id: &UncertaintyId) -> Option<&UncertaintyReference> {
        self.uncertainty_indices
            .get(id)
            .map(|index| &self.uncertainties[*index])
    }

    /// Look up a transform by its exact identity.
    pub fn transform(&self, id: &TransformId) -> Option<&FrameTransform> {
        self.transform_indices
            .get(id)
            .map(|index| &self.transforms[*index])
    }

    /// Look up a serial-section series by its owning volume frame.
    pub fn serial_section_series_for_volume(
        &self,
        volume: &CoordinateFrameId,
    ) -> Option<&SerialSectionSeries> {
        self.serial_series_indices
            .get(volume)
            .map(|index| &self.serial_section_series[*index])
    }

    /// Look up a serial section by its globally unique hierarchy identity.
    pub fn serial_section(&self, id: &SectionId) -> Option<&SerialSection> {
        self.serial_section_indices
            .get(id)
            .map(|&(series_index, section_index)| {
                &self.serial_section_series[series_index].sections()[section_index]
            })
    }

    /// Require a registered physical-space frame.
    pub fn require_physical(
        &self,
        id: &CoordinateFrameId,
    ) -> Result<&CoordinateFrame, CoordinateError> {
        let frame = self.require_frame(id)?;
        if frame.space() != CoordinateSpace::Physical {
            return Err(CoordinateError::SpaceMismatch { frame: id.clone() });
        }
        Ok(frame)
    }

    /// Require a registered frame with the requested dimension.
    pub fn require_dimension(
        &self,
        id: &CoordinateFrameId,
        expected: SpatialDimension,
    ) -> Result<&CoordinateFrame, CoordinateError> {
        let frame = self.require_frame(id)?;
        if frame.dimension() != expected {
            return Err(CoordinateError::DimensionMismatch {
                frame: id.clone(),
                expected,
                observed: frame.dimension(),
            });
        }
        Ok(frame)
    }

    /// Validate finite values and construct a two-dimensional coordinate.
    pub fn coordinate_2d(
        &self,
        frame: &CoordinateFrameId,
        values: [f64; 2],
    ) -> Result<Coordinate2D, CoordinateError> {
        self.require_dimension(frame, SpatialDimension::Two)?;
        validate_coordinate(frame, &values)?;
        Ok(Coordinate2D::new(frame.clone(), values))
    }

    /// Validate finite values and construct a three-dimensional coordinate.
    pub fn coordinate_3d(
        &self,
        frame: &CoordinateFrameId,
        values: [f64; 3],
    ) -> Result<Coordinate3D, CoordinateError> {
        self.require_dimension(frame, SpatialDimension::Three)?;
        validate_coordinate(frame, &values)?;
        Ok(Coordinate3D::new(frame.clone(), values))
    }

    /// Apply only the supplied ordered transform identities to a 2-D coordinate.
    pub fn apply_chain_2d(
        &self,
        coordinate: &Coordinate2D,
        chain: &[TransformId],
    ) -> Result<Coordinate2D, CoordinateError> {
        let mut frame = coordinate.frame().clone();
        let mut values = *coordinate.values();
        self.require_dimension(&frame, SpatialDimension::Two)?;

        for transform_id in chain {
            let transform = self.require_transform(transform_id)?;
            ensure_chain_source(transform, &frame)?;
            values = transform
                .matrix()
                .apply_2d(values)
                .expect("registry validates matrix dimensionality");
            frame = transform.target().clone();
            validate_coordinate(&frame, &values)?;
        }

        Ok(Coordinate2D::new(frame, values))
    }

    /// Apply only the supplied ordered transform identities to a 3-D coordinate.
    pub fn apply_chain_3d(
        &self,
        coordinate: &Coordinate3D,
        chain: &[TransformId],
    ) -> Result<Coordinate3D, CoordinateError> {
        let mut frame = coordinate.frame().clone();
        let mut values = *coordinate.values();
        self.require_dimension(&frame, SpatialDimension::Three)?;

        for transform_id in chain {
            let transform = self.require_transform(transform_id)?;
            ensure_chain_source(transform, &frame)?;
            values = transform
                .matrix()
                .apply_3d(values)
                .expect("registry validates matrix dimensionality");
            frame = transform.target().clone();
            validate_coordinate(&frame, &values)?;
        }

        Ok(Coordinate3D::new(frame, values))
    }

    /// Embed a placed 2-D section coordinate into its declared 3-D volume.
    pub fn embed_serial_section(
        &self,
        section_id: &SectionId,
        coordinate: &Coordinate2D,
    ) -> Result<Coordinate3D, CoordinateError> {
        let Some(&(series_index, section_index)) = self.serial_section_indices.get(section_id)
        else {
            return Err(CoordinateError::MissingSerialSection {
                section: section_id.clone(),
            });
        };
        let series = &self.serial_section_series[series_index];
        let section = &series.sections()[section_index];
        let Some(placement) = section.placement() else {
            return Err(CoordinateError::SerialSectionNotPlaced {
                section: section_id.clone(),
            });
        };
        if coordinate.frame() != placement.source_frame() {
            return Err(CoordinateError::SerialSectionCoordinateFrameMismatch {
                section: section_id.clone(),
                expected: placement.source_frame().clone(),
                observed: coordinate.frame().clone(),
            });
        }

        let semantic_xy = placement.apply_to_semantic_xy(*coordinate.values());
        let volume = self
            .frame(series.volume_frame())
            .expect("registry validates serial-section volume frames");
        let mut values = [0.0; 3];
        for (index, axis) in volume.axes().iter().enumerate() {
            values[index] = match axis {
                SpatialAxis::X => semantic_xy[0],
                SpatialAxis::Y => semantic_xy[1],
                SpatialAxis::Z => section.z_center(),
            };
        }
        self.coordinate_3d(series.volume_frame(), values)
    }

    fn require_frame(&self, id: &CoordinateFrameId) -> Result<&CoordinateFrame, CoordinateError> {
        self.frame(id)
            .ok_or_else(|| CoordinateError::MissingFrame { frame: id.clone() })
    }

    fn require_transform(&self, id: &TransformId) -> Result<&FrameTransform, CoordinateError> {
        self.transform(id)
            .ok_or_else(|| CoordinateError::MissingTransform {
                transform: id.clone(),
            })
    }
}

fn index_frames(
    frames: &[CoordinateFrame],
) -> Result<HashMap<CoordinateFrameId, usize>, CoordinateError> {
    let mut indices = HashMap::with_capacity(frames.len());
    for (index, frame) in frames.iter().enumerate() {
        if indices.insert(frame.id().clone(), index).is_some() {
            return Err(CoordinateError::DuplicateFrame {
                frame: frame.id().clone(),
            });
        }
    }
    Ok(indices)
}

fn index_uncertainties(
    uncertainties: &[UncertaintyReference],
) -> Result<HashMap<UncertaintyId, usize>, CoordinateError> {
    let mut indices = HashMap::with_capacity(uncertainties.len());
    for (index, uncertainty) in uncertainties.iter().enumerate() {
        if indices.insert(uncertainty.id().clone(), index).is_some() {
            return Err(CoordinateError::DuplicateUncertainty {
                uncertainty: uncertainty.id().clone(),
            });
        }
    }
    Ok(indices)
}

fn index_transforms(
    transforms: &[FrameTransform],
) -> Result<HashMap<TransformId, usize>, CoordinateError> {
    let mut indices = HashMap::with_capacity(transforms.len());
    for (index, transform) in transforms.iter().enumerate() {
        if indices.insert(transform.id().clone(), index).is_some() {
            return Err(CoordinateError::DuplicateTransform {
                transform: transform.id().clone(),
            });
        }
    }
    Ok(indices)
}

fn validate_uncertainty_frames(
    uncertainties: &[UncertaintyReference],
    frame_indices: &HashMap<CoordinateFrameId, usize>,
) -> Result<(), CoordinateError> {
    for uncertainty in uncertainties {
        if !frame_indices.contains_key(uncertainty.frame()) {
            return Err(CoordinateError::MissingUncertaintyFrame {
                uncertainty: uncertainty.id().clone(),
                frame: uncertainty.frame().clone(),
            });
        }
    }
    Ok(())
}

fn validate_transforms(
    transforms: &[FrameTransform],
    frames: &[CoordinateFrame],
    frame_indices: &HashMap<CoordinateFrameId, usize>,
    uncertainties: &[UncertaintyReference],
    uncertainty_indices: &HashMap<UncertaintyId, usize>,
) -> Result<Vec<(usize, usize)>, CoordinateError> {
    let mut endpoints = Vec::with_capacity(transforms.len());

    for transform in transforms {
        let Some(&source_index) = frame_indices.get(transform.source()) else {
            return Err(CoordinateError::MissingTransformSourceFrame {
                transform: transform.id().clone(),
                frame: transform.source().clone(),
            });
        };
        let Some(&target_index) = frame_indices.get(transform.target()) else {
            return Err(CoordinateError::MissingTransformTargetFrame {
                transform: transform.id().clone(),
                frame: transform.target().clone(),
            });
        };
        if source_index == target_index {
            return Err(CoordinateError::SelfTransform {
                transform: transform.id().clone(),
                frame: transform.source().clone(),
            });
        }

        let source_dimension = frames[source_index].dimension();
        let target_dimension = frames[target_index].dimension();
        if source_dimension != target_dimension
            || transform.matrix().dimension() != source_dimension
        {
            return Err(CoordinateError::TransformDimensionMismatch {
                transform: transform.id().clone(),
            });
        }

        if let Some(uncertainty_id) = transform.uncertainty() {
            let Some(&uncertainty_index) = uncertainty_indices.get(uncertainty_id) else {
                return Err(CoordinateError::MissingTransformUncertainty {
                    transform: transform.id().clone(),
                    uncertainty: uncertainty_id.clone(),
                });
            };
            let uncertainty = &uncertainties[uncertainty_index];
            if uncertainty.frame() != transform.target() {
                return Err(CoordinateError::TransformUncertaintyFrameMismatch {
                    transform: transform.id().clone(),
                    uncertainty: uncertainty_id.clone(),
                    expected: transform.target().clone(),
                    observed: uncertainty.frame().clone(),
                });
            }
        }

        endpoints.push((source_index, target_index));
    }

    Ok(endpoints)
}

fn index_serial_sections(
    series: &[SerialSectionSeries],
    frames: &[CoordinateFrame],
    frame_indices: &HashMap<CoordinateFrameId, usize>,
    uncertainties: &[UncertaintyReference],
    uncertainty_indices: &HashMap<UncertaintyId, usize>,
) -> Result<IndexedSerialSections, CoordinateError> {
    let mut series_indices = HashMap::with_capacity(series.len());
    let section_count = series.iter().map(|entry| entry.sections().len()).sum();
    let mut section_indices = HashMap::with_capacity(section_count);

    for (series_index, entry) in series.iter().enumerate() {
        if series_indices
            .insert(entry.volume_frame().clone(), series_index)
            .is_some()
        {
            return Err(CoordinateError::DuplicateSerialSectionSeries {
                volume: entry.volume_frame().clone(),
            });
        }
        let Some(&volume_index) = frame_indices.get(entry.volume_frame()) else {
            return Err(CoordinateError::MissingSerialSectionVolumeFrame {
                volume: entry.volume_frame().clone(),
            });
        };
        let volume = &frames[volume_index];
        if volume.dimension() != SpatialDimension::Three {
            return Err(CoordinateError::SerialSectionVolumeDimensionMismatch {
                volume: volume.id().clone(),
                observed: volume.dimension(),
            });
        }
        if volume.space() != CoordinateSpace::Physical {
            return Err(CoordinateError::SerialSectionVolumeSpaceMismatch {
                volume: volume.id().clone(),
            });
        }

        for (section_index, section) in entry.sections().iter().enumerate() {
            if section_indices
                .insert(section.id().clone(), (series_index, section_index))
                .is_some()
            {
                return Err(CoordinateError::DuplicateSerialSection {
                    section: section.id().clone(),
                });
            }
            let Some(placement) = section.placement() else {
                continue;
            };
            let Some(&source_index) = frame_indices.get(placement.source_frame()) else {
                return Err(CoordinateError::MissingSerialSectionSourceFrame {
                    section: section.id().clone(),
                    frame: placement.source_frame().clone(),
                });
            };
            let source = &frames[source_index];
            if source.dimension() != SpatialDimension::Two {
                return Err(CoordinateError::SerialSectionSourceDimensionMismatch {
                    section: section.id().clone(),
                    frame: source.id().clone(),
                    observed: source.dimension(),
                });
            }
            if source.space() != CoordinateSpace::Physical {
                return Err(CoordinateError::SerialSectionSourceSpaceMismatch {
                    section: section.id().clone(),
                    frame: source.id().clone(),
                });
            }
            if source.unit() != volume.unit() {
                return Err(CoordinateError::SerialSectionUnitMismatch {
                    section: section.id().clone(),
                    source_frame: source.id().clone(),
                    volume: volume.id().clone(),
                    source_unit: source.unit(),
                    volume_unit: volume.unit(),
                });
            }

            if let Some(uncertainty_id) = placement.uncertainty() {
                let Some(&uncertainty_index) = uncertainty_indices.get(uncertainty_id) else {
                    return Err(CoordinateError::MissingSerialSectionUncertainty {
                        section: section.id().clone(),
                        uncertainty: uncertainty_id.clone(),
                    });
                };
                let uncertainty = &uncertainties[uncertainty_index];
                if uncertainty.frame() != volume.id() {
                    return Err(CoordinateError::SerialSectionUncertaintyFrameMismatch {
                        section: section.id().clone(),
                        uncertainty: uncertainty_id.clone(),
                        expected: volume.id().clone(),
                        observed: uncertainty.frame().clone(),
                    });
                }
            }
        }
    }

    Ok(IndexedSerialSections {
        series: series_indices,
        sections: section_indices,
    })
}

fn reject_transform_cycles(
    frames: &[CoordinateFrame],
    transforms: &[FrameTransform],
    endpoints: &[(usize, usize)],
) -> Result<(), CoordinateError> {
    let mut outgoing = vec![Vec::<(usize, usize)>::new(); frames.len()];
    for (transform_index, &(source, target)) in endpoints.iter().enumerate() {
        outgoing[source].push((target, transform_index));
    }

    let mut colors = vec![0_u8; frames.len()];
    for start in 0..frames.len() {
        if colors[start] != 0 {
            continue;
        }
        colors[start] = 1;
        let mut stack = vec![(start, 0_usize)];
        while let Some(&(frame_index, next_edge)) = stack.last() {
            if next_edge == outgoing[frame_index].len() {
                colors[frame_index] = 2;
                stack.pop();
                continue;
            }

            stack.last_mut().expect("stack is non-empty").1 += 1;
            let (target_index, transform_index) = outgoing[frame_index][next_edge];
            match colors[target_index] {
                0 => {
                    colors[target_index] = 1;
                    stack.push((target_index, 0));
                }
                1 => {
                    let transform = &transforms[transform_index];
                    return Err(CoordinateError::TransformCycle {
                        transform: transform.id().clone(),
                        source_frame: transform.source().clone(),
                        target_frame: transform.target().clone(),
                    });
                }
                _ => {}
            }
        }
    }
    Ok(())
}

fn ensure_chain_source(
    transform: &FrameTransform,
    current_frame: &CoordinateFrameId,
) -> Result<(), CoordinateError> {
    if transform.source() != current_frame {
        return Err(CoordinateError::ChainSourceMismatch {
            transform: transform.id().clone(),
            expected: current_frame.clone(),
            observed: transform.source().clone(),
        });
    }
    Ok(())
}

fn validate_coordinate(frame: &CoordinateFrameId, values: &[f64]) -> Result<(), CoordinateError> {
    if let Some(index) = values.iter().position(|value| !value.is_finite()) {
        return Err(CoordinateError::NonFiniteCoordinate {
            frame: frame.clone(),
            index,
        });
    }
    Ok(())
}
