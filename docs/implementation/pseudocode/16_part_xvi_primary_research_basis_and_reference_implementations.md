# Part XVI — Primary Research Basis and Reference Implementations

The pseudocode above is synthesized from established primary methods, official mathematical formulations, and current reference implementations. The following are the principal sources; implementation tasks should pin exact software versions and licenses separately.

## Cohort-valid inference and comparison

1. Winkler AM, Ridgway GR, Webster MA, Smith SM, Nichols TE. **Permutation inference for the general linear model.** *NeuroImage*. 2014;92:381–397. DOI: https://doi.org/10.1016/j.neuroimage.2014.01.060
2. Myllymäki M, Mrkvička T, Grabarnik P, Seijo H, Hahn U. **Global envelope tests for spatial processes.** *Journal of the Royal Statistical Society: Series B*. 2017;79:381–404. DOI: https://doi.org/10.1111/rssb.12172
3. Gretton A, Borgwardt KM, Rasch MJ, Schölkopf B, Smola A. **A kernel two-sample test.** *JMLR*. 2012;13:723–773. https://jmlr.org/papers/v13/gretton12a.html
4. Székely GJ, Rizzo ML. **Energy statistics: A class of statistics based on distances.** *Journal of Statistical Planning and Inference*. 2013;143:1249–1272. DOI: https://doi.org/10.1016/j.jspi.2013.03.018
5. Schuirmann DJ. **A comparison of the two one-sided tests procedure and the power approach for assessing the equivalence of average bioavailability.** *Journal of Pharmacokinetics and Biopharmaceutics*. 1987;15:657–680. DOI: https://doi.org/10.1007/BF01068419
6. Saravanan V, Berman GJ, Sober SJ. **Application of the hierarchical bootstrap to multi-level data in neuroscience.** *Neuron, Behavior and Data Analysis*. 2020. https://doi.org/10.51628/001c.13927

## Bayesian spatial modeling

7. Lindgren F, Rue H, Lindström J. **An explicit link between Gaussian fields and Gaussian Markov random fields: the stochastic partial differential equation approach.** *JRSS B*. 2011;73:423–498. DOI: https://doi.org/10.1111/j.1467-9868.2011.00777.x
8. Rue H, Martino S, Chopin N. **Approximate Bayesian inference for latent Gaussian models by using integrated nested Laplace approximations.** *JRSS B*. 2009;71:319–392. DOI: https://doi.org/10.1111/j.1467-9868.2008.00700.x
9. Quiñonero-Candela J, Rasmussen CE. **A unifying view of sparse approximate Gaussian process regression.** *JMLR*. 2005;6:1939–1959. https://www.jmlr.org/papers/v6/quinonero-candela05a.html
10. Snelson E, Ghahramani Z. **Sparse Gaussian processes using pseudo-inputs.** *NeurIPS*. 2005.
11. Datta A, Banerjee S, Finley AO, Gelfand AE. **Hierarchical nearest-neighbor Gaussian process models for large geostatistical datasets.** *JASA*. 2016;111:800–812. DOI: https://doi.org/10.1080/01621459.2015.1044091
12. Besag J. **Spatial interaction and the statistical analysis of lattice systems.** *JRSS B*. 1974;36:192–236. DOI: https://doi.org/10.1111/j.2517-6161.1974.tb00999.x
13. Besag J, York J, Mollié A. **Bayesian image restoration, with two applications in spatial statistics.** *Annals of the Institute of Statistical Mathematics*. 1991;43:1–20. DOI: https://doi.org/10.1007/BF00116466
14. Riebler A, Sørbye SH, Simpson D, Rue H. **An intuitive Bayesian spatial model for disease mapping that accounts for scaling.** *Statistical Methods in Medical Research*. 2016;25:1145–1165. DOI: https://doi.org/10.1177/0962280216660421
15. Whittle P. **On stationary processes in the plane.** *Biometrika*. 1954;41:434–449. DOI: https://doi.org/10.1093/biomet/41.3-4.434
16. Hoffman MD, Gelman A. **The No-U-Turn sampler: adaptively setting path lengths in Hamiltonian Monte Carlo.** *JMLR*. 2014;15:1593–1623. https://jmlr.org/papers/v15/hoffman14a.html
17. Del Moral P, Doucet A, Jasra A. **Sequential Monte Carlo samplers.** *JRSS B*. 2006;68:411–436. DOI: https://doi.org/10.1111/j.1467-9868.2006.00553.x
18. Kucukelbir A, Tran D, Ranganath R, Gelman A, Blei DM. **Automatic differentiation variational inference.** *JMLR*. 2017;18:1–45. https://jmlr.org/papers/v18/16-107.html
19. Vehtari A, Gelman A, Gabry J. **Practical Bayesian model evaluation using leave-one-out cross-validation and WAIC.** *Statistics and Computing*. 2017;27:1413–1432. DOI: https://doi.org/10.1007/s11222-016-9696-4
20. Vehtari A, Gelman A, Simpson D, Carpenter B, Bürkner P-C. **Rank-normalization, folding, and localization: an improved R-hat for assessing convergence of MCMC.** *Bayesian Analysis*. 2021;16:667–718. DOI: https://doi.org/10.1214/20-BA1221
21. Talts S, Betancourt M, Simpson D, Vehtari A, Gelman A. **Validating Bayesian inference algorithms with simulation-based calibration.** 2018. arXiv:1804.06788.

## Point processes

22. Møller J, Syversveen AR, Waagepetersen RP. **Log Gaussian Cox processes.** *Scandinavian Journal of Statistics*. 1998;25:451–482. DOI: https://doi.org/10.1111/1467-9469.00115
23. Strauss DJ. **A model for clustering.** *Biometrika*. 1975;62:467–475. DOI: https://doi.org/10.1093/biomet/62.2.467
24. Baddeley A, Turner R. **Practical maximum pseudolikelihood for spatial point patterns.** *Australian & New Zealand Journal of Statistics*. 2000;42:283–322. DOI: https://doi.org/10.1111/1467-842X.00128
25. Murray I, Ghahramani Z, MacKay DJC. **MCMC for doubly-intractable distributions.** *UAI*. 2006.
26. Geyer CJ, Møller J. **Simulation procedures and likelihood inference for spatial point processes.** *Scandinavian Journal of Statistics*. 1994;21:359–373.
27. Berthelsen KK, Møller J. **Likelihood and non-parametric Bayesian MCMC inference for spatial point processes based on perfect simulation and path sampling.** *Scandinavian Journal of Statistics*. 2003;30:549–564. DOI: https://doi.org/10.1111/1467-9469.00348

## Graph and higher-order mathematics

28. Sandryhaila A, Moura JMF. **Discrete signal processing on graphs.** *IEEE Transactions on Signal Processing*. 2013;61:1644–1656. DOI: https://doi.org/10.1109/TSP.2013.2238935
29. Hammond DK, Vandergheynst P, Gribonval R. **Wavelets on graphs via spectral graph theory.** *Applied and Computational Harmonic Analysis*. 2011;30:129–150. DOI: https://doi.org/10.1016/j.acha.2010.04.005
30. Coifman RR, Maggioni M. **Diffusion wavelets.** *Applied and Computational Harmonic Analysis*. 2006;21:53–94. DOI: https://doi.org/10.1016/j.acha.2006.04.004
31. Defferrard M, Bresson X, Vandergheynst P. **Convolutional neural networks on graphs with fast localized spectral filtering.** *NeurIPS*. 2016.
32. Gama F, Ribeiro A, Bruna J. **Diffusion scattering transforms on graphs.** *ICLR*. 2019.
33. Zhou D, Huang J, Schölkopf B. **Learning with hypergraphs: clustering, classification, and embedding.** *NeurIPS*. 2006.
34. Benson AR, Gleich DF, Leskovec J. **Higher-order organization of complex networks.** *Science*. 2016;353:163–166. DOI: https://doi.org/10.1126/science.aad9029
35. Schaub MT, Benson AR, Horn P, Lippner G, Jadbabaie A. **Random walks on simplicial complexes and the normalized Hodge 1-Laplacian.** *SIAM Review*. 2020;62:353–391. DOI: https://doi.org/10.1137/18M1201019
36. Barbarossa S, Sardellitti S. **Topological signal processing over simplicial complexes.** *IEEE Transactions on Signal Processing*. 2020;68:2992–3007. DOI: https://doi.org/10.1109/TSP.2020.2981920

## Registration and optimal transport

37. Beg MF, Miller MI, Trouvé A, Younes L. **Computing large deformation metric mappings via geodesic flows of diffeomorphisms.** *International Journal of Computer Vision*. 2005;61:139–157. DOI: https://doi.org/10.1023/B:VISI.0000043755.93987.aa
38. Avants BB, Epstein CL, Grossman M, Gee JC. **Symmetric diffeomorphic image registration with cross-correlation.** *Medical Image Analysis*. 2008;12:26–41. DOI: https://doi.org/10.1016/j.media.2007.06.004
39. Balakrishnan G, Zhao A, Sabuncu MR, Guttag J, Dalca AV. **VoxelMorph: a learning framework for deformable medical image registration.** *IEEE Transactions on Medical Imaging*. 2019;38:1788–1800. DOI: https://doi.org/10.1109/TMI.2019.2897538
40. Dalca AV, Balakrishnan G, Guttag J, Sabuncu MR. **Unsupervised learning of probabilistic diffeomorphic registration for images and surfaces.** *Medical Image Analysis*. 2019;57:226–236. DOI: https://doi.org/10.1016/j.media.2019.07.006
41. Cuturi M. **Sinkhorn distances: lightspeed computation of optimal transport.** *NeurIPS*. 2013.
42. Chizat L, Peyré G, Schmitzer B, Vialard F-X. **Scaling algorithms for unbalanced optimal transport problems.** *Mathematics of Computation*. 2018;87:2563–2609. DOI: https://doi.org/10.1090/mcom/3303
43. Vayer T, Chapel L, Flamary R, Tavenard R, Courty N. **Optimal transport for structured data with application on graphs.** *ICML*. 2019.
44. Chapel L, Alaya MZ, Gasso G. **Partial optimal transport with applications on positive-unlabeled learning.** *NeurIPS*. 2020.
45. Marsland S, Shardlow T. **Bayesian uncertainty quantification for image registration.** *SIAM/ASA Journal on Uncertainty Quantification*. 2017;5:100–131. DOI: https://doi.org/10.1137/16M1079282

## Topology

46. Edelsbrunner H, Letscher D, Zomorodian A. **Topological persistence and simplification.** *Discrete & Computational Geometry*. 2002;28:511–533. DOI: https://doi.org/10.1007/s00454-002-2885-2
47. Bubenik P. **Statistical topological data analysis using persistence landscapes.** *JMLR*. 2015;16:77–102. https://jmlr.org/papers/v16/bubenik15a.html
48. Adams H, Emerson T, Kirby M, et al. **Persistence images: a stable vector representation of persistent homology.** *JMLR*. 2017;18:1–35. https://jmlr.org/papers/v18/16-337.html
49. Edelsbrunner H, Mücke EP. **Three-dimensional alpha shapes.** *ACM Transactions on Graphics*. 1994;13:43–72. DOI: https://doi.org/10.1145/174462.156635
50. de Silva V, Carlsson G. **Topological estimation using witness complexes.** *Symposium on Point-Based Graphics*. 2004. DOI: https://doi.org/10.2312/SPBG/SPBG04/157-166
51. Cohen-Steiner D, Edelsbrunner H, Harer J. **Stability of persistence diagrams.** *Discrete & Computational Geometry*. 2007;37:103–120. DOI: https://doi.org/10.1007/s00454-006-1276-5

## Multimodal Bayesian modeling

52. Bach FR, Jordan MI. **A probabilistic interpretation of canonical correlation analysis.** Technical Report 688, UC Berkeley. 2005.
53. Argelaguet R, Velten B, Arnol D, et al. **Multi-Omics Factor Analysis—a framework for unsupervised integration of multi-omics data sets.** *Molecular Systems Biology*. 2018;14:e8124. DOI: https://doi.org/10.15252/msb.20178124
54. Argelaguet R, Arnol D, Bredikhin D, et al. **MOFA+: a statistical framework for comprehensive integration of multi-modal single-cell data.** *Genome Biology*. 2020;21:111. DOI: https://doi.org/10.1186/s13059-020-02015-1
55. Hogan JW, Tchernis R. **Bayesian factor analysis for spatially correlated data, with application to summarizing area-level material deprivation from census data.** *Journal of the American Statistical Association*. 2004;99:314–324. DOI: https://doi.org/10.1198/016214504000000296
56. Wang F, Wall MM. **Generalized common spatial factor model.** *Biostatistics*. 2003;4:569–582. DOI: https://doi.org/10.1093/biostatistics/4.4.569
57. Zhang L, Banerjee S. **Spatial factor modeling: A Bayesian matrix-normal approach for misaligned data.** *Biometrics*. 2022;78:560–573. DOI: https://doi.org/10.1111/biom.13452

## Mechanistic and simulation-based inference

58. Turing AM. **The chemical basis of morphogenesis.** *Philosophical Transactions of the Royal Society B*. 1952;237:37–72. DOI: https://doi.org/10.1098/rstb.1952.0012
59. Fisher RA. **The wave of advance of advantageous genes.** *Annals of Eugenics*. 1937;7:355–369. DOI: https://doi.org/10.1111/j.1469-1809.1937.tb02153.x
60. Anderson ARA, Chaplain MAJ. **Continuous and discrete mathematical models of tumor-induced angiogenesis.** *Bulletin of Mathematical Biology*. 1998;60:857–899. DOI: https://doi.org/10.1006/bulm.1998.0042
61. Gatenby RA, Gawlinski ET. **A reaction-diffusion model of cancer invasion.** *Cancer Research*. 1996;56:5745–5753.
62. Wood SN. **Statistical inference for noisy nonlinear ecological dynamic systems.** *Nature*. 2010;466:1102–1104. DOI: https://doi.org/10.1038/nature09319
63. Papamakarios G, Sterratt D, Murray I. **Sequential neural likelihood: fast likelihood-free inference with autoregressive flows.** *AISTATS*. 2019. arXiv:1805.07226.
64. Durkan C, Murray I, Papamakarios G. **On contrastive learning for likelihood-free inference.** *ICML*. 2020.
65. Talts S, Betancourt M, Simpson D, Vehtari A, Gelman A. **Validating Bayesian inference algorithms with simulation-based calibration.** arXiv:1804.06788.
66. Mei H, Eisner J. **The neural Hawkes process: a neurally self-modulating multivariate point process.** *NeurIPS*. 2017.
67. Luo S, Hu W. **Diffusion probabilistic models for 3D point cloud generation.** *CVPR*. 2021. arXiv:2103.01458.

## Causal interference and active design

68. Hudgens MG, Halloran ME. **Toward causal inference with interference.** *JASA*. 2008;103:832–842. DOI: https://doi.org/10.1198/016214508000000292
69. Aronow PM, Samii C. **Estimating average causal effects under general interference, with application to a social network experiment.** *Annals of Applied Statistics*. 2017;11:1912–1947. DOI: https://doi.org/10.1214/16-AOAS1005
70. Chernozhukov V, Chetverikov D, Demirer M, et al. **Double/debiased machine learning for treatment and structural parameters.** *The Econometrics Journal*. 2018;21:C1–C68. DOI: https://doi.org/10.1111/ectj.12097
71. Manski CF. **Identification of treatment response with social interactions.** *The Econometrics Journal*. 2013;16:S1–S23. DOI: https://doi.org/10.1111/j.1368-423X.2012.00368.x
72. Rosenbaum PR. **Sensitivity analysis for certain permutation inferences in matched observational studies.** *Biometrika*. 1987;74:13–26. DOI: https://doi.org/10.1093/biomet/74.1.13
73. Lipsitch M, Tchetgen Tchetgen E, Cohen T. **Negative controls: a tool for detecting confounding and bias in observational studies.** *Epidemiology*. 2010;21:383–388. DOI: https://doi.org/10.1097/EDE.0b013e3181d61eeb
74. Chaloner K, Verdinelli I. **Bayesian experimental design: a review.** *Statistical Science*. 1995;10:273–304. DOI: https://doi.org/10.1214/ss/1177009939
75. Kleinegesse S, Drovandi C, Gutmann MU. **Sequential Bayesian experimental design for implicit models via mutual information.** 2020. arXiv:2003.09379.

## Reference implementations to use as numerical or interoperability oracles

- `spatstat` / R for point processes, windows, simulations, and functional summaries.
- Stan/CmdStan, PyMC, NumPyro, Turing, or equivalent pinned backends for Bayesian differential checks.
- R-INLA/inlabru for SPDE/INLA-style latent Gaussian models.
- GPyTorch/GPflow or equivalent pinned implementations for scalable GP comparisons.
- POT for entropic, unbalanced, partial, and fused optimal-transport comparison.
- GUDHI, Ripser, Dionysus, or a selected pinned TDA implementation for persistence fixtures.
- PyTorch Geometric/DGL or pinned sparse linear-algebra references for graph-learning interoperability.
- `sbi` or a pinned SBI backend for NPE/NLE/NRE differential tests.
- ANTs/SyN, LDDMM, or selected registration backends for deformation and uncertainty fixtures.

These packages are comparators and backends, not semantic authorities by name alone. Marklab must match the chosen estimator, normalization, boundary rule, likelihood, prior, and numerical mode before claiming equivalence.

---

