use marklab_cohort::{
    multisite_covariate_patient_contrast, MultisiteCovariatePatientRecord, MultisiteEffectModel,
    MultisiteInferenceSpec,
};

#[test]
fn adjusted_site_effects_and_fixed_pool_match_an_independent_qr_fwl_oracle() {
    let records = records();
    let spec = MultisiteInferenceSpec {
        model: MultisiteEffectModel::FixedEffect,
        alpha: 0.05,
    };
    let result = multisite_covariate_patient_contrast(&records, "A", "B", &spec).expect("result");
    let mut reversed = records.clone();
    reversed.reverse();
    assert_eq!(
        result,
        multisite_covariate_patient_contrast(&reversed, "A", "B", &spec).expect("reordered result")
    );
    let mut weighted_effect = 0.0;
    let mut total_weight = 0.0;
    for site in &result.sites {
        let rows = records
            .iter()
            .filter(|record| record.site_id == site.site_id)
            .collect::<Vec<_>>();
        let nuisance = rows
            .iter()
            .map(|record| vec![1.0, record.covariates[0], record.covariates[1]])
            .collect::<Vec<_>>();
        let group = rows
            .iter()
            .map(|record| f64::from(record.group == "A"))
            .collect::<Vec<_>>();
        let outcome = rows
            .iter()
            .map(|record| record.endpoint)
            .collect::<Vec<_>>();
        let (effect, standard_error) = fwl(&nuisance, &group, &outcome);
        assert!((site.effect - effect).abs() < 1e-12);
        assert!((site.standard_error - standard_error).abs() < 1e-12);
        let weight = 1.0 / standard_error.powi(2);
        weighted_effect += weight * effect;
        total_weight += weight;
    }
    assert!((result.pooled.pooled_effect - weighted_effect / total_weight).abs() < 1e-12);
    assert!((result.pooled.pooled_standard_error - (1.0 / total_weight).sqrt()).abs() < 1e-12);
}

#[test]
fn adjusted_multisite_rejects_incomplete_and_site_collinear_nuisance_designs() {
    let spec = MultisiteInferenceSpec {
        model: MultisiteEffectModel::FixedEffect,
        alpha: 0.05,
    };
    let mut incomplete = records();
    incomplete[0].covariate_names.pop();
    incomplete[0].covariates.pop();
    assert!(multisite_covariate_patient_contrast(&incomplete, "A", "B", &spec).is_err());

    let mut collinear = records();
    for record in collinear
        .iter_mut()
        .filter(|record| record.site_id == "site-b")
    {
        record.covariates[1] = 2.0 * record.covariates[0];
    }
    assert!(matches!(
        multisite_covariate_patient_contrast(&collinear, "A", "B", &spec),
        Err(error) if error.to_string().contains("site site-b")
            && error.to_string().contains("rank deficient")
    ));
}

fn records() -> Vec<MultisiteCovariatePatientRecord> {
    let noise = [-1.0, 0.2, 0.8, -0.4];
    ["site-a", "site-b", "site-c"]
        .into_iter()
        .enumerate()
        .flat_map(|(site_index, site)| {
            ["A", "B"].into_iter().flat_map(move |group| {
                noise.into_iter().enumerate().map(move |(index, noise)| {
                    let batch = (index % 2) as f64;
                    MultisiteCovariatePatientRecord {
                        patient_id: format!("{site}-{group}-{}", index + 1),
                        site_id: site.into(),
                        group: group.into(),
                        endpoint: site_index as f64
                            + 2.0 * index as f64
                            + 0.5 * batch
                            + noise
                            + if group == "A" { 3.0 } else { 0.0 },
                        covariate_names: vec!["age".into(), "batch".into()],
                        covariates: vec![index as f64, batch],
                    }
                })
            })
        })
        .collect()
}

fn fwl(nuisance: &[Vec<f64>], group: &[f64], outcome: &[f64]) -> (f64, f64) {
    let group_fit = qr_fitted(nuisance, group);
    let outcome_fit = qr_fitted(nuisance, outcome);
    let group_residual = group
        .iter()
        .zip(group_fit)
        .map(|(value, fit)| value - fit)
        .collect::<Vec<_>>();
    let outcome_residual = outcome
        .iter()
        .zip(outcome_fit)
        .map(|(value, fit)| value - fit)
        .collect::<Vec<_>>();
    let denominator = dot(&group_residual, &group_residual);
    let coefficient = dot(&group_residual, &outcome_residual) / denominator;
    let rss = outcome_residual
        .iter()
        .zip(&group_residual)
        .map(|(outcome, group)| (outcome - coefficient * group).powi(2))
        .sum::<f64>();
    let degrees = outcome.len() - nuisance[0].len() - 1;
    (coefficient, (rss / degrees as f64 / denominator).sqrt())
}

fn qr_fitted(design: &[Vec<f64>], outcome: &[f64]) -> Vec<f64> {
    let columns = design[0].len();
    let mut q = Vec::<Vec<f64>>::with_capacity(columns);
    let mut r = vec![vec![0.0; columns]; columns];
    for column in 0..columns {
        let mut vector = design.iter().map(|row| row[column]).collect::<Vec<_>>();
        for prior in 0..column {
            r[prior][column] = dot(&q[prior], &vector);
            for (value, basis) in vector.iter_mut().zip(&q[prior]) {
                *value -= r[prior][column] * basis;
            }
        }
        r[column][column] = dot(&vector, &vector).sqrt();
        assert!(r[column][column] > 1e-12);
        for value in &mut vector {
            *value /= r[column][column];
        }
        q.push(vector);
    }
    let qty = q
        .iter()
        .map(|column| dot(column, outcome))
        .collect::<Vec<_>>();
    let mut beta = vec![0.0; columns];
    for row in (0..columns).rev() {
        let remainder = ((row + 1)..columns)
            .map(|column| r[row][column] * beta[column])
            .sum::<f64>();
        beta[row] = (qty[row] - remainder) / r[row][row];
    }
    design.iter().map(|row| dot(row, &beta)).collect()
}

fn dot(left: &[f64], right: &[f64]) -> f64 {
    left.iter()
        .zip(right)
        .map(|(left, right)| left * right)
        .sum()
}
