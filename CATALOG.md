# Catálogo de Plug-ins — Bulldog Complements

Todos os identificadores OpenFX seguem o prefixo `br.com.bulldog.complements.<suite>.<efeito>` e são validados automaticamente pela CI (`bulldog_packager validate-catalog`).

| Suíte | Bundle | Identificador OpenFX | Nome de Exibição | Contexto | RoD/RoI | IsIdentity | Backend | Status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| **Continuus** | `BulldogContinuus.ofx.bundle` | `br.com.bulldog.complements.continuus.soft_blur` | Bulldog Soft Blur | Filter | Expande | Sim (`radius=0` ou `mix=0`) | CPU (rayon) | Implementado |
| **Sapphirus** | `BulldogSapphirus.ofx.bundle` | `br.com.bulldog.complements.sapphirus.optical_glow` | Bulldog Optical Glow | Filter | Expande | Sim (`intensity=0`) | CPU (rayon) | Implementado |
| **Dulcia Nitentia** | `BulldogDulciaNitentia.ofx.bundle` | `br.com.bulldog.complements.dulcia_nitentia.optical_vignette` | Bulldog Optical Vignette | Filter | Preserva | Sim (`strength=0` e `grain=0`) | CPU (rayon) | Implementado |
| **Caeruleus Nebulosus** | `BulldogCaeruleusNebulosus.ofx.bundle` | `br.com.bulldog.complements.caeruleus_nebulosus.letterbox_matte` | Bulldog Letterbox Matte | Filter | Preserva | Sim (`bar_height=0` ou `opacity=0`) | CPU (rayon) | Implementado |
| **3D Transitions** | `BulldogTransitions3d.ofx.bundle` | `br.com.bulldog.complements.transitions_3d.cube_spin` | Bulldog 3D Cube Spin | Transition | União | Sim (`0.0`→From, `1.0`→To) | CPU (rayon) | Implementado |
| **TrackerMan** | `BulldogTrackerMan.ofx.bundle` | `br.com.bulldog.complements.tracker_man.planar_transform` | Bulldog Planar Transform | Filter | Preserva | Sim (transformação neutra) | CPU (rayon) | Implementado |
| **Glass Brokers** | `BulldogGlassBrokers.ofx.bundle` | `br.com.bulldog.complements.glass_brokers.facet_wipe` | Bulldog Facet Wipe 3D | Transition | União | Sim (`0.0`→From, `1.0`→To) | CPU (rayon) | Implementado |
| **Universum** | `BulldogUniversum.ofx.bundle` | `br.com.bulldog.complements.universum.signal_drift` | Bulldog Signal Drift | Filter | Preserva | Sim (`intensity=0`) | CPU (rayon) | Implementado |
| **Particulae Legales** | `BulldogParticulaeLegales.ofx.bundle` | `br.com.bulldog.complements.particulae_legales.ambient_embers` | Bulldog Ambient Embers | Filter | Preserva | Sim (`density=0`) | CPU (rayon) | Implementado |
