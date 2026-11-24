# Bulldog Complements

Coleção open-source de plug-ins nativos **OpenFX (OFX 1.4)** de efeitos visuais, óptica, color grading, partículas, utilitários e transições 3D escrita em **Rust nativo**, desenvolvida em **sala limpa (*clean room*)** sob licença dual `MIT OR Apache-2.0`.

## Princípios de Engenharia

- **Zero Panic em Produção:** Nenhum `unwrap()`, `expect()`, `panic!()`, indexação não verificada ou unwind atravessando a fronteira C ABI (`catch_unwind` obrigatório em todos os pontos de entrada OpenFX).
- **Pipeline Óptico Linear `f32`:** Processamento interno em ponto flutuante linear com preservação de canal alfa (pré-multiplicado e reto), suporte nativo a RGBA 8-bit, 16-bit e 32-bit float, e suporte completo a `row_bytes` (*strides*) positivos, com padding e negativos.
- **Independência de Resolução e Determinismo:** Parâmetros espaciais normalizados e efeitos temporais (ruído, grão, partículas, transições) 100% determinísticos para acesso aleatório na timeline.
- **Desempenho em Qualquer Hardware:** Paralelismo seguro por linhas com `rayon`, pool de buffers reutilizáveis por instância, cancelamento cooperativo (`OfxAbort`) e arquitetura pronta para aceleração GPU com fallback obrigatório em CPU.
- **Um `.ofx.bundle` por Suíte:** Identificadores OpenFX sob o namespace `br.com.bulldog.complements.*`.

## Suítes da Coleção

| Suíte | Crate | Bundle OpenFX | Domínio |
| --- | --- | --- | --- |
| **Continuus** | `continuus` | `BulldogContinuus.ofx.bundle` | Acabamento, desfoque, estilização, distorção, restauração e compositing multiuso |
| **Sapphirus** | `sapphirus` | `BulldogSapphirus.ofx.bundle` | Óptica de precisão, glows HDR, lens flares procedurais, aberração cromática e luz volumétrica |
| **Dulcia Nitentia** | `dulcia_nitentia` | `BulldogDulciaNitentia.ofx.bundle` | Color grading, emulação de película, curvas tonais, halation, vinhetas ópticas e grão físico |
| **Caeruleus Nebulosus** | `caeruleus_nebulosus` | `BulldogCaeruleusNebulosus.ofx.bundle` | Utilitários de produção, mattes/enquadramento, títulos, PiP e ajustes rápidos |
| **3D Transitions** | `transitions_3d` | `BulldogTransitions3d.ofx.bundle` | Transições espaciais e geométricas 3D em perspectiva (`OfxImageEffectContextTransition`) |
| **TrackerMan** | `tracker_man` | `BulldogTrackerMan.ofx.bundle` | Rastreamento planar, transformações projetivas (*corner pin*), estabilização e rotoscopia |
| **Glass Brokers** | `glass_brokers` | `BulldogGlassBrokers.ofx.bundle` | Facetas 3D, fraturas, extrusões, refração e transições geométricas estilizadas |
| **Universum** | `universum` | `BulldogUniversum.ofx.bundle` | Estilização retrô/analógica (CRT, sinal, varredura, glitch), HUDs e motion graphics |
| **Particulae Legales** | `particulae_legales` | `BulldogParticulaeLegales.ofx.bundle` | Sistemas determinísticos de partículas procedurais (brasas, fumaça, faíscas, atmosferas) |

Consulte a lista completa de plug-ins implementados e seus identificadores OpenFX em [`CATALOG.md`](CATALOG.md).

## Compilação, Testes e Empacotamento `.ofx.bundle`

Requer Rust 1.85+ (edição 2024).

```bash
# Verificações de qualidade
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

# Auditoria de catálogo e licenças SPDX
cargo run -p bulldog_packager -- validate-catalog
cargo run -p bulldog_packager -- check-licenses

# Build release e empacotamento dos 9 bundles .ofx.bundle em dist/
cargo build --workspace --release
cargo run -p bulldog_packager -- package --profile release --out-dir dist
```

### Instalação em Hosts OpenFX

Copie as pastas `dist/*.ofx.bundle` para o diretório padrão OpenFX do sistema operacional:

- **Windows:** `C:\Program Files\Common Files\OFX\Plugins\`
- **Linux:** `/usr/OFX/Plugins/`
- **macOS:** `/Library/OFX/Plugins/`

## Governança e Licença

- Regras de engenharia e sala limpa: [`AGENTS.md`](AGENTS.md)
- Guia prático de desenvolvimento: [`CLAUDE.md`](CLAUDE.md)
- Registro de atribuições e assets: [`ATTRIBUTION.md`](ATTRIBUTION.md)
- Licença: dual [`MIT`](LICENSE-MIT) OR [`Apache-2.0`](LICENSE-APACHE).
