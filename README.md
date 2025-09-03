# Elementary Monte Carlo model of the anisotropic recrystallization and “anti-ripening” under intensive stirring and high supersaturations.
The production of fibrous oxides, particularly V2O5, by intensive stirring in water is examined through the framework of driven systems, an approach developed in the 1980s by Georges Martin et al. for systems under irradiation or severe plastic deformation. Instead of ballistic diffusion, the model introduces ballistic detachments of atoms from the oxide surface under stirring. A simplified Monte Carlo scheme is proposed for crystal evolution within the Terrace-Ledge-Kink (TLK) model. This scheme accounts for anisotropy and additional athermal detachment probabilities. An individual cluster within a limited volume becomes elongated in a steady state or dissolves. For an ensemble of clusters, the total number decreases (similar to common ripening), but the mean length of the fibers grows. This leads to an increase in the total surface energy, which is contrary to the behavior observed in common ripening.

The code used for these experiments is located in the repository.

The foundational single-crystal model is located in the `RustCode/model_1_001` directory. An expanded version, which enables the simulation of a cluster ensemble, is available in `RustCode/model_1_002`. All subsequent data processing and analytical scripts are collected within the `PythonCode` directory, which represents the complete experimental codebase.

## Citations
If our work is useful for your research, please consider citing and give us a star ⭐:
```
@article{abakumov2025elementarymontecarlomodel,
      title={Elementary Monte Carlo model of the anisotropic recrystallization and antiripening under intensive stirring and high supersaturations}, 
      author={Serhii Abakumov and Eugen Rabkin and Andriy Gusak},
      year={2025},
      eprint={2508.13799},
      archivePrefix={arXiv},
      primaryClass={cond-mat.mtrl-sci},
      url={https://arxiv.org/abs/2508.13799}, 
}
```

## Contact
Mail for contacts: `abakumov.serhii.official@gmail.com`.