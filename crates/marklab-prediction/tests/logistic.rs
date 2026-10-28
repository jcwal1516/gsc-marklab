use marklab_prediction::{
    area_under_roc, average_precision, fit_and_evaluate_logistic_ensemble, ClassWeight,
    ConformalCall, LogisticEnsemble, LogisticEnsembleSpec, LogisticError, LogisticProblem,
    PlattScaling,
};

fn table(rows: usize, columns: usize, value: impl Fn(usize, usize) -> f64) -> Vec<f64> {
    (0..rows)
        .flat_map(|row| (0..columns).map(move |column| (row, column)))
        .map(|(row, column)| value(row, column))
        .collect()
}

fn assert_probabilities(problem: &LogisticProblem, c: f64, test: &[f64], expected: &[f64]) {
    let model = problem.fit(c).unwrap();
    let columns = test.len() / expected.len();
    for (row, expected) in test.chunks_exact(columns).zip(expected) {
        let actual = model.probability(row).unwrap();
        assert!(
            (actual - expected).abs() < 1e-7,
            "probability {actual} differs from scikit-learn's {expected}"
        );
    }
}

// Expected probabilities from scikit-learn 1.9.0: StandardScaler() followed by
// LogisticRegression(C, class_weight, solver="newton-cholesky", tol=1e-14) on the same tables.
// Its lbfgs solver agrees within 1e-8.
#[test]
fn l2_logistic_regression_matches_scikit_learn() {
    // More features than rows (solved in the row space), balanced class weights, and a constant
    // column whose test value differs from training.
    let features = table(10, 14, |i, j| {
        if j == 5 {
            2.5
        } else {
            (1.3 * i as f64 + 0.7 * j as f64).sin() + 0.1 * j as f64 * (i % 3) as f64
        }
    });
    let labels = [1, 0, 0, 1, 0, 1, 0, 0, 1, 0].map(|y| y == 1);
    let problem = LogisticProblem::new(&features, 14, &labels, ClassWeight::Balanced).unwrap();
    let test = table(3, 14, |t, j| {
        if j == 5 {
            7.0
        } else {
            (0.9 * t as f64 + 0.4 * j as f64).cos()
        }
    });
    assert_probabilities(
        &problem,
        0.5,
        &test,
        &[0.3981220065771219, 0.3457388245362915, 0.33208059021143255],
    );

    // More rows than features, unweighted.
    let features = table(30, 4, |i, j| {
        (0.37 * i as f64 * (j + 1) as f64).sin() + 0.02 * i as f64 * j as f64
    });
    let labels = (0..30)
        .map(|i| (2.1 * i as f64).sin() + 0.8 * features[i * 4] > 0.0)
        .collect::<Vec<_>>();
    let problem = LogisticProblem::new(&features, 4, &labels, ClassWeight::Uniform).unwrap();
    let test = table(3, 4, |t, j| (0.5 * t as f64 + j as f64).cos());
    assert_probabilities(
        &problem,
        2.0,
        &test,
        &[0.6534872332138117, 0.5903672852930246, 0.48766732694537784],
    );
}

#[test]
fn ranking_metrics_match_scikit_learn_with_tied_scores() {
    let labels = [1, 0, 1, 1, 0, 0, 1, 0].map(|y| y == 1);
    let scores = [0.9, 0.8, 0.8, 0.4, 0.4, 0.2, 0.1, 0.1];
    // roc_auc_score and average_precision_score in scikit-learn 1.9.0.
    assert_eq!(area_under_roc(&labels, &scores), Some(0.65625));
    let precision = average_precision(&labels, &scores).unwrap();
    assert!((precision - 0.6916666666666667).abs() < 1e-15);
    assert_eq!(area_under_roc(&[true, true], &[0.1, 0.2]), None);
}

#[test]
fn platt_scaling_matches_the_smoothed_target_maximum_likelihood() {
    let margins = [-2.1, -1.3, -0.4, 0.2, 0.9, 1.7, 2.5, -0.8, 0.3, 1.1];
    let labels = [0, 0, 1, 0, 1, 1, 1, 0, 0, 1].map(|y| y == 1);
    let platt = PlattScaling::fit(&margins, &labels).unwrap();
    // Newton's method on the same objective in 50-digit arithmetic (mpmath). scikit-learn
    // 1.9.0's _sigmoid_calibration gives -A, -B within 2e-8 of these.
    assert!(
        (platt.slope - 0.967_956_738_671_143_7).abs() < 1e-12,
        "{platt:?}"
    );
    assert!(
        (platt.intercept - -0.207_913_561_977_038).abs() < 1e-12,
        "{platt:?}"
    );
}

fn grouped_data(groups: usize) -> (Vec<f64>, Vec<bool>, Vec<usize>) {
    // Groups of two rows; a group's rows share a label and lie close together. The first two
    // features carry a signal that overlaps between the classes.
    let rows = 2 * groups;
    let group_of: Vec<usize> = (0..rows).map(|row| row / 2).collect();
    let labels: Vec<bool> = group_of.iter().map(|group| group % 3 == 0).collect();
    let features = table(rows, 6, |row, column| {
        let signal = if labels[row] { 0.8 } else { -0.8 };
        let group = group_of[row] as f64;
        signal * f64::from(u8::from(column < 2))
            + 1.5 * (1.7 * group + column as f64).sin()
            + 0.05 * (row % 2) as f64
    });
    (features, labels, group_of)
}

fn spec(seed: u64) -> LogisticEnsembleSpec {
    LogisticEnsembleSpec {
        c_values: vec![0.01, 0.1, 1.0, 10.0],
        class_weight: ClassWeight::Balanced,
        folds: 3,
        inner_folds: 3,
        seed,
        significance: 0.2,
    }
}

#[test]
fn ensemble_evaluation_keeps_groups_together_covers_each_class_and_repeats_from_its_seed() {
    let (features, labels, groups) = grouped_data(60);
    let (evaluation, fit) =
        fit_and_evaluate_logistic_ensemble(&features, 6, &labels, &groups, &spec(7)).unwrap();
    assert_eq!(evaluation.predictions.len(), labels.len());
    for (row, prediction) in evaluation.predictions.iter().enumerate() {
        assert_eq!(prediction.row, row);
        assert_eq!(
            prediction.fold,
            evaluation.predictions[row ^ 1].fold,
            "group split"
        );
    }
    for fold in &evaluation.folds {
        assert_eq!(fold.training_rows + fold.test_rows, labels.len());
        assert_eq!(fold.heldout_mean_auroc.len(), 4);
    }
    assert!(
        evaluation.pooled_auroc > 0.75,
        "signal: {}",
        evaluation.pooled_auroc
    );
    // Class-conditional cross-conformal sets cover each true class at least 1 − 2ε of the time.
    let positive = &evaluation.conformal_positive_rows;
    let negative = &evaluation.conformal_negative_rows;
    for (covered, class_rows) in [
        (positive.positive + positive.both, 40),
        (negative.negative + negative.both, 80),
    ] {
        let coverage = covered as f64 / class_rows as f64;
        assert!(
            coverage >= 0.6,
            "coverage {coverage}: {positive:?} {negative:?}"
        );
    }
    assert_eq!(fit.ensemble.members(), 3);
    assert_eq!(
        fit_and_evaluate_logistic_ensemble(&features, 6, &labels, &groups, &spec(7)).unwrap(),
        (evaluation.clone(), fit)
    );
    let (reshuffled, _) =
        fit_and_evaluate_logistic_ensemble(&features, 6, &labels, &groups, &spec(8)).unwrap();
    assert!(reshuffled
        .predictions
        .iter()
        .zip(&evaluation.predictions)
        .any(|(a, b)| a.fold != b.fold));
}

#[test]
fn a_saved_ensemble_predicts_identically_and_tampering_is_rejected() {
    let (features, labels, groups) = grouped_data(30);
    let (_, fit) =
        fit_and_evaluate_logistic_ensemble(&features, 6, &labels, &groups, &spec(3)).unwrap();
    let saved = serde_json::to_value(&fit.ensemble).unwrap();
    let loaded: LogisticEnsemble = serde_json::from_value(saved.clone()).unwrap();
    for row in features.as_chunks::<6>().0.iter().take(5) {
        assert_eq!(
            loaded.predict(row).unwrap(),
            fit.ensemble.predict(row).unwrap()
        );
    }
    let prediction = loaded.predict(&features[..6]).unwrap();
    let expected_call = match (
        prediction.p_value_positive > 0.2,
        prediction.p_value_negative > 0.2,
    ) {
        (true, false) => ConformalCall::Positive,
        (false, true) => ConformalCall::Negative,
        (true, true) => ConformalCall::Both,
        (false, false) => ConformalCall::Neither,
    };
    assert_eq!(prediction.call, expected_call);
    assert!(
        loaded.predict(&features[..5]).is_err(),
        "wrong feature count"
    );

    // A row far beyond every calibration row of one class is maximally typical of that class
    // and maximally atypical of the other: p-values 1 and 1 / (rows of the other class + 1).
    let extreme = |signal: f64| [signal, signal, 0.0, 0.0, 0.0, 0.0];
    let positive = loaded.predict(&extreme(20.0)).unwrap();
    assert_eq!(positive.p_value_positive, 1.0);
    assert_eq!(positive.p_value_negative, 1.0 / 41.0);
    assert_eq!(positive.call, ConformalCall::Positive);
    let negative = loaded.predict(&extreme(-20.0)).unwrap();
    assert_eq!(negative.p_value_negative, 1.0);
    assert_eq!(negative.p_value_positive, 1.0 / 21.0);
    assert_eq!(negative.call, ConformalCall::Negative);

    let mut unsorted = saved.clone();
    unsorted["positive_margins"][0] = serde_json::json!([1.0, -1.0]);
    let mut short = saved;
    short["members"][0]["scale"] = serde_json::json!([1.0]);
    for tampered in [unsorted, short] {
        assert!(serde_json::from_value::<LogisticEnsemble>(tampered).is_err());
    }
}

#[test]
fn ensemble_fitting_rejects_unusable_settings() {
    let (features, labels, groups) = grouped_data(15);
    let cases: [(&str, LogisticEnsembleSpec); 4] = [
        (
            "strictly increasing",
            LogisticEnsembleSpec {
                c_values: vec![1.0, 1.0],
                ..spec(1)
            },
        ),
        (
            "folds must be between 2 and 20",
            LogisticEnsembleSpec {
                folds: 1,
                ..spec(1)
            },
        ),
        (
            "significance must lie strictly between 0 and 1",
            LogisticEnsembleSpec {
                significance: 1.0,
                ..spec(1)
            },
        ),
        (
            "5 groups are positive, fewer than the 6 folds",
            LogisticEnsembleSpec {
                folds: 6,
                ..spec(1)
            },
        ),
    ];
    for (message, spec) in cases {
        match fit_and_evaluate_logistic_ensemble(&features, 6, &labels, &groups, &spec) {
            Err(LogisticError::Invalid(actual)) => {
                assert!(actual.contains(message), "{actual:?} lacks {message:?}")
            }
            other => panic!("expected rejection containing {message:?}, got {other:?}"),
        }
    }
}
