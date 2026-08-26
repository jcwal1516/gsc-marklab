# MM-JOINT-01 — Modular joint pathology compile and fit

Own `CompileJointPathologyModel` and `FitJointPathologyModel` together through IC-0175. The
missing-command red preceded implementation. The four-patient/eight-region oracle compiles five
typed measured likelihood blocks, improves the Laplace objective, and predicts one masked clinical
outcome below RMSE 0.6. The model is frontier, synthetic, narrow, and `approximate_only`.
