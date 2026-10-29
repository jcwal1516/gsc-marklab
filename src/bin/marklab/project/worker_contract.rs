use super::{
    BackendContract, CliError, Codec, ContentDigest, Cow, NodeContract, PYMC_VERSION,
    PYTHON_VERSION,
};
use marklab_core::backend::is_sha256;

#[derive(Clone, Copy)]
pub(super) enum StaticBackendWorkflow {
    PymcNormalMean,
    PymcHierarchicalNormal,
    PymcBetaBinomialHierarchy,
    PymcBetaBinomialGroupRegression,
    PymcDirichletMultinomialGroup,
    PymcOrdinalGroup,
    PymcHurdleBetaBinomialGroup,
    PymcOrdinalGroupSiteHierarchy,
    PymcNonproportionalOrdinalGroupSite,
    PymcBetaBinomialGroupGenderRegression,
    PymcBetaBinomialGroupGenderSlideHierarchy,
    PymcStudentTHierarchy,
    PymcGriddedLgcp,
    PymcArbitraryWindowIpp,
    PymcArbitraryWindowIppSpatialPpc,
    PymcArbitraryWindowLgcp,
    PymcReplicatedArbitraryWindowLgcp,
    PymcReplicatedArbitraryWindowLgcpInferredKernel,
    PymcReplicatedArbitraryWindowMultitypeLgcpInferredKernel,
    PymcReplicatedArbitraryWindowMultitypeLgcp,
    PymcConditionalMultitypeMark,
    PymcReplicatedConditionalMultitypeMark,
    PotFusedGromovWasserstein,
}

#[derive(Clone, Copy)]
pub(super) struct StaticBackendDescriptor {
    pub(super) backend_id: &'static str,
    pub(super) backend_version: &'static str,
    pub(super) python_version: &'static str,
    pub(super) license: &'static str,
    pub(super) input_kinds: &'static [&'static str],
    pub(super) output_kind: &'static str,
    pub(super) result_schema_id: &'static str,
    pub(super) node_id: &'static str,
    pub(super) node_kind: &'static str,
    pub(super) implementation_identity: &'static str,
    pub(super) deterministic_controls: &'static str,
}

impl StaticBackendWorkflow {
    pub(super) fn descriptor(self) -> StaticBackendDescriptor {
        match self {
            Self::PymcNormalMean => StaticBackendDescriptor {
                backend_id: "pymc",
                backend_version: PYMC_VERSION,
                python_version: PYTHON_VERSION,
                license: "Apache-2.0",
                input_kinds: &["application/vnd.marklab.source.normal-mean-observations;version=1"],
                output_kind: "application/vnd.marklab.pymc-worker-result+json;version=1",
                result_schema_id: "marklab.pymc_worker_result",
                node_id: "pymc-normal-mean",
                node_kind: "bayesian_fit",
                implementation_identity: "marklab-project-pymc-normal-mean-node-v1",
                deterministic_controls: "seeded-nuts-request",
            },
            Self::PymcHierarchicalNormal => StaticBackendDescriptor {
                backend_id: "pymc",
                backend_version: PYMC_VERSION,
                python_version: PYTHON_VERSION,
                license: "Apache-2.0",
                input_kinds: &[
                    "application/vnd.marklab.source.hierarchical-normal-observations;version=1",
                ],
                output_kind:
                    "application/vnd.marklab.pymc-hierarchical-worker-result+json;version=2",
                result_schema_id: "marklab.pymc_hierarchical_worker_result",
                node_id: "pymc-hierarchical-normal",
                node_kind: "bayesian_hierarchical_fit",
                implementation_identity: "marklab-project-pymc-hierarchical-normal-node-v2",
                deterministic_controls: "seeded-nuts-hierarchical-request",
            },
            Self::PymcBetaBinomialHierarchy => StaticBackendDescriptor {
                backend_id: "pymc",
                backend_version: PYMC_VERSION,
                python_version: PYTHON_VERSION,
                license: "Apache-2.0",
                input_kinds: &[
                    "application/vnd.marklab.source.beta-binomial-patient-counts;version=1",
                ],
                output_kind:
                    "application/vnd.marklab.pymc-beta-binomial-hierarchy-worker-result+json;version=2",
                result_schema_id: "marklab.pymc_beta_binomial_hierarchy_worker_result",
                node_id: "pymc-beta-binomial-hierarchy",
                node_kind: "bayesian_count_hierarchical_fit",
                implementation_identity: "marklab-project-pymc-beta-binomial-hierarchy-node-v2",
                deterministic_controls: "seeded-nuts-beta-binomial-hierarchical-request",
            },
            Self::PymcBetaBinomialGroupRegression => StaticBackendDescriptor {
                backend_id: "pymc",
                backend_version: PYMC_VERSION,
                python_version: PYTHON_VERSION,
                license: "Apache-2.0",
                input_kinds: &[
                    "application/vnd.marklab.source.beta-binomial-patient-group-counts;version=1",
                ],
                output_kind:
                    "application/vnd.marklab.pymc-beta-binomial-group-regression-worker-result+json;version=2",
                result_schema_id: "marklab.pymc_beta_binomial_group_regression_worker_result",
                node_id: "pymc-beta-binomial-group-regression",
                node_kind: "bayesian_count_group_regression_fit",
                implementation_identity:
                    "marklab-project-pymc-beta-binomial-group-regression-node-v2",
                deterministic_controls: "seeded-nuts-beta-binomial-group-regression-request",
            },
            Self::PymcDirichletMultinomialGroup => StaticBackendDescriptor {
                backend_id: "pymc",
                backend_version: PYMC_VERSION,
                python_version: PYTHON_VERSION,
                license: "Apache-2.0",
                input_kinds: &[
                    "application/vnd.marklab.source.dirichlet-multinomial-patient-group-counts;version=1",
                ],
                output_kind:
                    "application/vnd.marklab.pymc-dirichlet-multinomial-group-worker-result+json;version=2",
                result_schema_id: "marklab.pymc_dirichlet_multinomial_group_worker_result",
                node_id: "pymc-dirichlet-multinomial-group",
                node_kind: "bayesian_multiclass_count_group_regression_fit",
                implementation_identity:
                    "marklab-project-pymc-dirichlet-multinomial-group-node-v2",
                deterministic_controls: "seeded-nuts-dirichlet-multinomial-group-request",
            },
            Self::PymcOrdinalGroup => StaticBackendDescriptor {
                backend_id: "pymc",
                backend_version: PYMC_VERSION,
                python_version: PYTHON_VERSION,
                license: "Apache-2.0",
                input_kinds: &[
                    "application/vnd.marklab.source.ordinal-patient-group-outcomes;version=1",
                ],
                output_kind:
                    "application/vnd.marklab.pymc-ordinal-group-worker-result+json;version=1",
                result_schema_id: "marklab.pymc_ordinal_group_worker_result",
                node_id: "pymc-ordinal-group",
                node_kind: "bayesian_ordinal_group_regression_fit",
                implementation_identity: "marklab-project-pymc-ordinal-group-node-v1",
                deterministic_controls: "seeded-nuts-ordinal-group-request",
            },
            Self::PymcHurdleBetaBinomialGroup => StaticBackendDescriptor {
                backend_id: "pymc",
                backend_version: PYMC_VERSION,
                python_version: PYTHON_VERSION,
                license: "Apache-2.0",
                input_kinds: &[
                    "application/vnd.marklab.source.hurdle-beta-binomial-patient-group-counts;version=1",
                ],
                output_kind:
                    "application/vnd.marklab.pymc-hurdle-beta-binomial-group-worker-result+json;version=1",
                result_schema_id: "marklab.pymc_hurdle_beta_binomial_group_worker_result",
                node_id: "pymc-hurdle-beta-binomial-group",
                node_kind: "bayesian_hurdle_count_group_regression_fit",
                implementation_identity:
                    "marklab-project-pymc-hurdle-beta-binomial-group-node-v1",
                deterministic_controls: "seeded-nuts-hurdle-beta-binomial-group-request",
            },
            Self::PymcOrdinalGroupSiteHierarchy => StaticBackendDescriptor {
                backend_id: "pymc", backend_version: PYMC_VERSION, python_version: PYTHON_VERSION,
                license: "Apache-2.0",
                input_kinds: &["application/vnd.marklab.source.ordinal-patient-group-site-outcomes;version=1"],
                output_kind: "application/vnd.marklab.pymc-ordinal-group-site-hierarchy-worker-result+json;version=1",
                result_schema_id: "marklab.pymc_ordinal_group_site_hierarchy_worker_result",
                node_id: "pymc-ordinal-group-site-hierarchy", node_kind: "bayesian_ordinal_site_hierarchy_fit",
                implementation_identity: "marklab-project-pymc-ordinal-group-site-hierarchy-node-v1",
                deterministic_controls: "seeded-nuts-ordinal-group-site-hierarchy-request",
            },
            Self::PymcNonproportionalOrdinalGroupSite => StaticBackendDescriptor {
                backend_id:"pymc",backend_version: PYMC_VERSION,python_version: PYTHON_VERSION,license:"Apache-2.0",
                input_kinds:&["application/vnd.marklab.source.nonproportional-ordinal-patient-group-site-outcomes;version=1"],
                output_kind:"application/vnd.marklab.pymc-nonproportional-ordinal-group-site-worker-result+json;version=1",
                result_schema_id:"marklab.pymc_nonproportional_ordinal_group_site_worker_result",
                node_id:"pymc-nonproportional-ordinal-group-site",node_kind:"bayesian_nonproportional_ordinal_site_fit",
                implementation_identity:"marklab-project-pymc-nonproportional-ordinal-group-site-node-v1",
                deterministic_controls:"seeded-nuts-nonproportional-ordinal-group-site-request",
            },
            Self::PymcBetaBinomialGroupGenderRegression => StaticBackendDescriptor {
                backend_id: "pymc",
                backend_version: PYMC_VERSION,
                python_version: PYTHON_VERSION,
                license: "Apache-2.0",
                input_kinds: &[
                    "application/vnd.marklab.source.beta-binomial-patient-group-gender-counts;version=1",
                ],
                output_kind:
                    "application/vnd.marklab.pymc-beta-binomial-group-gender-regression-worker-result+json;version=2",
                result_schema_id:
                    "marklab.pymc_beta_binomial_group_gender_regression_worker_result",
                node_id: "pymc-beta-binomial-group-gender-regression",
                node_kind: "bayesian_count_group_gender_regression_fit",
                implementation_identity:
                    "marklab-project-pymc-beta-binomial-group-gender-regression-node-v2",
                deterministic_controls:
                    "seeded-nuts-beta-binomial-group-gender-regression-request",
            },
            Self::PymcBetaBinomialGroupGenderSlideHierarchy => StaticBackendDescriptor {
                backend_id: "pymc",
                backend_version: PYMC_VERSION,
                python_version: PYTHON_VERSION,
                license: "Apache-2.0",
                input_kinds: &[
                    "application/vnd.marklab.source.beta-binomial-group-gender-slide-counts;version=1",
                ],
                output_kind:
                    "application/vnd.marklab.pymc-beta-binomial-group-gender-slide-hierarchy-worker-result+json;version=2",
                result_schema_id:
                    "marklab.pymc_beta_binomial_group_gender_slide_hierarchy_worker_result",
                node_id: "pymc-beta-binomial-group-gender-slide-hierarchy",
                node_kind: "bayesian_repeated_slide_patient_hierarchical_fit",
                implementation_identity:
                    "marklab-project-pymc-beta-binomial-group-gender-slide-hierarchy-node-v2",
                deterministic_controls:
                    "seeded-nuts-beta-binomial-group-gender-slide-hierarchy-request",
            },
            Self::PymcStudentTHierarchy => StaticBackendDescriptor {
                backend_id: "pymc",
                backend_version: PYMC_VERSION,
                python_version: PYTHON_VERSION,
                license: "Apache-2.0",
                input_kinds: &[
                    "application/vnd.marklab.source.student-t-hierarchy-observations;version=1",
                ],
                output_kind:
                    "application/vnd.marklab.pymc-student-t-hierarchy-worker-result+json;version=2",
                result_schema_id: "marklab.pymc_student_t_hierarchy_worker_result",
                node_id: "pymc-student-t-hierarchy",
                node_kind: "bayesian_robust_hierarchical_fit",
                implementation_identity: "marklab-project-pymc-student-t-hierarchy-node-v2",
                deterministic_controls: "seeded-nuts-student-t-hierarchical-request",
            },
            Self::PymcGriddedLgcp => StaticBackendDescriptor {
                backend_id: "pymc",
                backend_version: PYMC_VERSION,
                python_version: PYTHON_VERSION,
                license: "Apache-2.0",
                input_kinds: &[
                    "application/vnd.marklab.source.point-events;version=1",
                    "application/vnd.marklab.source.gridded-covariates;version=1",
                ],
                output_kind:
                    "application/vnd.marklab.pymc-gridded-lgcp-worker-result+json;version=2",
                result_schema_id: "marklab.pymc_gridded_lgcp_worker_result",
                node_id: "pymc-gridded-lgcp",
                node_kind: "bayesian_point_process_field_fit",
                implementation_identity: "marklab-project-pymc-gridded-lgcp-node-v2",
                deterministic_controls: "seeded-nuts-fixed-grid-lgcp-request",
            },
            Self::PymcArbitraryWindowIpp => StaticBackendDescriptor {
                backend_id: "pymc",
                backend_version: PYMC_VERSION,
                python_version: PYTHON_VERSION,
                license: "Apache-2.0",
                input_kinds: &[
                    "application/vnd.marklab.source.arbitrary-window-ipp-events+csv;version=1",
                    "application/vnd.marklab.source.arbitrary-window-ipp-quadrature+csv;version=1",
                    "application/vnd.marklab.source.observation-window+geojson;version=1",
                ],
                output_kind:
                    "application/vnd.marklab.pymc-arbitrary-window-ipp-fit+json;version=1",
                result_schema_id: "marklab.pymc_arbitrary_window_ipp_fit_result",
                node_id: "pymc-arbitrary-window-ipp-fit",
                node_kind: "bayesian_point_process_fit",
                implementation_identity: "marklab-project-pymc-arbitrary-window-ipp-fit-node-v1",
                deterministic_controls: "seeded-nuts-weighted-exact-window-ipp-request",
            },
            Self::PymcArbitraryWindowIppSpatialPpc => StaticBackendDescriptor {
                backend_id: "pymc",
                backend_version: PYMC_VERSION,
                python_version: PYTHON_VERSION,
                license: "Apache-2.0",
                input_kinds: &[
                    "application/vnd.marklab.source.arbitrary-window-ipp-events+csv;version=1",
                    "application/vnd.marklab.source.arbitrary-window-ipp-membership+csv;version=1",
                    "application/vnd.marklab.source.arbitrary-window-ipp-quadrature+csv;version=1",
                    "application/vnd.marklab.source.observation-window+geojson;version=1",
                ],
                output_kind:
                    "application/vnd.marklab.pymc-arbitrary-window-ipp-spatial-ppc+json;version=1",
                result_schema_id: "marklab.pymc_arbitrary_window_ipp_spatial_ppc_result",
                node_id: "pymc-arbitrary-window-ipp-spatial-ppc",
                node_kind: "bayesian_point_process_spatial_posterior_predictive_check",
                implementation_identity:
                    "marklab-project-pymc-arbitrary-window-ipp-spatial-ppc-node-v1",
                deterministic_controls:
                    "seeded-nuts-and-posterior-predictive-exact-window-ipp-request",
            },
            Self::PymcArbitraryWindowLgcp => StaticBackendDescriptor {
                backend_id: "pymc",
                backend_version: PYMC_VERSION,
                python_version: PYTHON_VERSION,
                license: "Apache-2.0",
                input_kinds: &[
                    "application/vnd.marklab.source.arbitrary-window-lgcp-events+csv;version=1",
                    "application/vnd.marklab.source.arbitrary-window-lgcp-membership+csv;version=1",
                    "application/vnd.marklab.source.arbitrary-window-lgcp-quadrature+csv;version=1",
                    "application/vnd.marklab.source.observation-window+geojson;version=1",
                ],
                output_kind:
                    "application/vnd.marklab.pymc-arbitrary-window-lgcp-fit+json;version=2",
                result_schema_id: "marklab.pymc_arbitrary_window_lgcp_fit_result",
                node_id: "pymc-arbitrary-window-lgcp-fit",
                node_kind: "bayesian_point_process_latent_field_fit",
                implementation_identity: "marklab-project-pymc-arbitrary-window-lgcp-fit-node-v2",
                deterministic_controls:
                    "seeded-nuts-weighted-exact-window-fixed-matern-lgcp-request",
            },
            Self::PymcReplicatedArbitraryWindowLgcp => StaticBackendDescriptor {
                backend_id: "pymc",
                backend_version: PYMC_VERSION,
                python_version: PYTHON_VERSION,
                license: "Apache-2.0",
                input_kinds: &[
                    "application/vnd.marklab.source.replicated-arbitrary-window-lgcp+csv;version=1",
                ],
                output_kind:
                    "application/vnd.marklab.pymc-replicated-arbitrary-window-lgcp-fit+json;version=3",
                result_schema_id:
                    "marklab.pymc_replicated_arbitrary_window_lgcp_fit_result_v2",
                node_id: "pymc-replicated-arbitrary-window-lgcp-fit",
                node_kind: "bayesian_replicated_point_process_latent_field_fit",
                implementation_identity:
                    "marklab-project-pymc-replicated-arbitrary-window-lgcp-fit-node-v3",
                deterministic_controls:
                    "seeded-nuts-patient-pattern-hierarchy-fixed-matern-request",
            },
            Self::PymcReplicatedArbitraryWindowLgcpInferredKernel => StaticBackendDescriptor {
                backend_id: "pymc",
                backend_version: PYMC_VERSION,
                python_version: PYTHON_VERSION,
                license: "Apache-2.0",
                input_kinds: &[
                    "application/vnd.marklab.source.replicated-arbitrary-window-lgcp+csv;version=1",
                ],
                output_kind:
                    "application/vnd.marklab.pymc-replicated-arbitrary-window-lgcp-inferred-kernel-fit+json;version=2",
                result_schema_id:
                    "marklab.pymc_replicated_arbitrary_window_lgcp_inferred_kernel_fit_result",
                node_id: "pymc-replicated-arbitrary-window-lgcp-inferred-kernel-fit",
                node_kind: "bayesian_replicated_point_process_inferred_latent_field_fit",
                implementation_identity:
                    "marklab-project-pymc-replicated-arbitrary-window-lgcp-inferred-kernel-fit-node-v2",
                deterministic_controls:
                    "seeded-nuts-patient-pattern-hierarchy-inferred-shared-matern-request",
            },
            Self::PymcReplicatedArbitraryWindowMultitypeLgcpInferredKernel => StaticBackendDescriptor {
                backend_id: "pymc",
                backend_version: PYMC_VERSION,
                python_version: PYTHON_VERSION,
                license: "Apache-2.0",
                input_kinds: &[
                    "application/vnd.marklab.source.replicated-arbitrary-window-multitype-lgcp+csv;version=1",
                ],
                output_kind:
                    "application/vnd.marklab.pymc-replicated-arbitrary-window-multitype-lgcp-inferred-kernel-fit+json;version=2",
                result_schema_id:
                    "marklab.pymc_replicated_arbitrary_window_multitype_lgcp_inferred_kernel_fit_result",
                node_id: "pymc-replicated-arbitrary-window-multitype-lgcp-inferred-kernel-fit",
                node_kind: "bayesian_replicated_multitype_point_process_inferred_latent_field_fit",
                implementation_identity:
                    "marklab-project-pymc-replicated-arbitrary-window-multitype-lgcp-inferred-kernel-fit-node-v2",
                deterministic_controls:
                    "seeded-nuts-patient-pattern-type-hierarchy-inferred-shared-matern-request",
            },
            Self::PymcReplicatedArbitraryWindowMultitypeLgcp => StaticBackendDescriptor {
                backend_id: "pymc",
                backend_version: PYMC_VERSION,
                python_version: PYTHON_VERSION,
                license: "Apache-2.0",
                input_kinds: &[
                    "application/vnd.marklab.source.replicated-arbitrary-window-multitype-lgcp+csv;version=1",
                ],
                output_kind:
                    "application/vnd.marklab.pymc-replicated-arbitrary-window-multitype-lgcp-fit+json;version=2",
                result_schema_id:
                    "marklab.pymc_replicated_arbitrary_window_multitype_lgcp_fit_result",
                node_id: "pymc-replicated-arbitrary-window-multitype-lgcp-fit",
                node_kind: "bayesian_replicated_multitype_point_process_latent_field_fit",
                implementation_identity:
                    "marklab-project-pymc-replicated-arbitrary-window-multitype-lgcp-fit-node-v2",
                deterministic_controls:
                    "seeded-nuts-patient-pattern-type-hierarchy-fixed-matern-request",
            },
            Self::PymcConditionalMultitypeMark => StaticBackendDescriptor {
                backend_id: "pymc",
                backend_version: PYMC_VERSION,
                python_version: PYTHON_VERSION,
                license: "Apache-2.0",
                input_kinds: &[
                    "application/vnd.marklab.source.conditional-multitype-mark+csv;version=1",
                ],
                output_kind:
                    "application/vnd.marklab.pymc-conditional-multitype-mark-fit+json;version=2",
                result_schema_id: "marklab.conditional_multitype_mark_fit",
                node_id: "pymc-conditional-multitype-mark-fit",
                node_kind: "bayesian_conditional_multitype_mark_fit",
                implementation_identity:
                    "marklab-project-pymc-conditional-multitype-mark-fit-node-v2",
                deterministic_controls:
                    "seeded-nuts-fixed-location-radius-graph-conditional-mark-request",
            },
            Self::PymcReplicatedConditionalMultitypeMark => StaticBackendDescriptor {
                backend_id: "pymc",
                backend_version: PYMC_VERSION,
                python_version: PYTHON_VERSION,
                license: "Apache-2.0",
                input_kinds: &[
                    "application/vnd.marklab.source.replicated-conditional-multitype-mark+csv;version=1",
                ],
                output_kind:
                    "application/vnd.marklab.pymc-replicated-conditional-multitype-mark-fit+json;version=2",
                result_schema_id: "marklab.replicated_conditional_multitype_mark_fit",
                node_id: "pymc-replicated-conditional-multitype-mark-fit",
                node_kind: "bayesian_patient_replicated_conditional_multitype_mark_fit",
                implementation_identity:
                    "marklab-project-pymc-replicated-conditional-multitype-mark-fit-node-v3",
                deterministic_controls:
                    "seeded-nuts-patient-pattern-fixed-location-conditional-mark-request",
            },
            Self::PotFusedGromovWasserstein => StaticBackendDescriptor {
                backend_id: "pot",
                backend_version: "0.9.7.post1",
                python_version: PYTHON_VERSION,
                license: "MIT",
                input_kinds: &["application/vnd.marklab.source.fgw-input+json;version=1"],
                output_kind:
                    "application/vnd.marklab.pot-fused-gromov-worker-result+json;version=1",
                result_schema_id: "marklab.pot_fused_gromov_wasserstein_worker_result",
                node_id: "pot-fused-gromov-wasserstein",
                node_kind: "descriptive_alignment",
                implementation_identity: "marklab-project-pot-fused-gromov-node-v2",
                deterministic_controls: "fixed-initialization-order-and-solver-controls",
            },
        }
    }
}

impl StaticBackendDescriptor {
    pub(super) fn validate_request_backend(
        self,
        backend: &BackendContract,
    ) -> Result<(), CliError> {
        if backend.name != self.backend_id
            || backend.version != self.backend_version
            || backend.python_version != self.python_version
            || !is_sha256(&backend.environment_lock_sha256)
            || !is_sha256(&backend.worker_sha256)
        {
            return Err(CliError::Input(format!(
                "{} durable backend identity is incomplete or differs from its static descriptor",
                self.backend_id
            )));
        }
        Ok(())
    }

    pub(super) fn configuration_digest(
        self,
        backend: &BackendContract,
        request_bytes: &[u8],
    ) -> ContentDigest {
        let mut frames = vec![
            b"marklab-static-durable-backend-v1".as_slice(),
            self.backend_id.as_bytes(),
            self.backend_version.as_bytes(),
            self.python_version.as_bytes(),
            self.license.as_bytes(),
            backend.environment_lock_sha256.as_bytes(),
            backend.worker_sha256.as_bytes(),
        ];
        frames.extend(self.input_kinds.iter().map(|kind| kind.as_bytes()));
        frames.extend([
            self.result_schema_id.as_bytes(),
            self.deterministic_controls.as_bytes(),
            request_bytes,
        ]);
        ContentDigest::from_framed(frames)
    }

    pub(super) fn execution_policy(self) -> Vec<u8> {
        format!(
            "marklab-static-process-policy-v1\0backend={}\0version={}\0python={}\0license={}\0controls={}\0environment=cleared\0network=cleared\0bounded-process=true",
            self.backend_id,
            self.backend_version,
            self.python_version,
            self.license,
            self.deterministic_controls
        )
        .into_bytes()
    }

    pub(super) fn result_schema(self) -> (&'static str, u32) {
        (self.result_schema_id, 1)
    }

    /// Node contract for a worker request whose backend matches this descriptor.
    pub(super) fn contract(
        self,
        backend: &BackendContract,
        request_bytes: &[u8],
        codec: Codec,
    ) -> Result<NodeContract, CliError> {
        self.validate_request_backend(backend)?;
        Ok(NodeContract {
            id: self.node_id,
            kind: self.node_kind,
            version: 1,
            configuration_digest: self.configuration_digest(backend, request_bytes),
            execution_policy: Cow::Owned(self.execution_policy()),
            implementation_identity: self.implementation_identity,
            output_kind: self.output_kind,
            codec,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn backend(
        name: &'static str,
        version: &'static str,
        lock_byte: char,
        worker_byte: char,
    ) -> BackendContract {
        BackendContract {
            name,
            version,
            python_version: PYTHON_VERSION,
            environment_lock_sha256: lock_byte.to_string().repeat(64),
            worker_sha256: worker_byte.to_string().repeat(64),
        }
    }

    #[test]
    fn static_backend_identity_binds_environment_worker_and_request_bytes() {
        let descriptor = StaticBackendWorkflow::PymcNormalMean.descriptor();
        let exact = backend("pymc", "6.3.0", 'a', 'b');
        descriptor
            .validate_request_backend(&exact)
            .expect("exact descriptor");
        let baseline = descriptor.configuration_digest(&exact, b"request-a");

        let changed_lock = backend("pymc", "6.3.0", 'c', 'b');
        let changed_worker = backend("pymc", "6.3.0", 'a', 'd');
        assert_ne!(
            baseline,
            descriptor.configuration_digest(&changed_lock, b"request-a")
        );
        assert_ne!(
            baseline,
            descriptor.configuration_digest(&changed_worker, b"request-a")
        );
        assert_ne!(
            baseline,
            descriptor.configuration_digest(&exact, b"request-b")
        );

        let wrong_backend = backend("pot", "0.9.7.post1", 'a', 'b');
        assert!(descriptor.validate_request_backend(&wrong_backend).is_err());
    }
}
