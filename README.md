# Lemmings Amiga — moteur Rust expérimental

Projet local créé à partir des deux ADF présents dans ce dossier. L'application macOS utilise les décors, objets, sprites, sons et partitions de ces disques. Le moteur de jeu est écrit en Rust, avec Macroquad pour la fenêtre, le rendu et la sortie audio. Il s'agit d'une première version jouable ; ce n'est pas encore une reproduction intégrale et fidèle de toutes les règles Amiga.

## Version 0.1.2 — audit de toutes les compétences

Les associations sont maintenant centralisées dans `src/presentation.rs` pour l'export, le rendu et la capture CPU. `verify` contrôle les instructions 68000 sélectionnant les 17 séquences, puis compare les 337 images exportées aux plans et masques du programme original.

| Compétence | Images et transitions | Sons |
|---|---|---|
| Climber | Escalade 8 images par sens, passage du rebord 8 images par sens ajouté | Attribution ; aucune boucle sonore ajoutée |
| Floater | 8 images par sens, ouverture puis oscillation du parachute | Attribution ; aucune boucle sonore ajoutée |
| Bomber | Décompte 5 secondes, séquence « Oh no » 16 images puis flash d'explosion 32 × 32 | Attribution, voix puis explosion |
| Blocker | Blocage 16 images, cycle réinitialisé à l'attribution | Attribution |
| Builder | Construction 16 images par sens, fin de tâche 8 images par sens ajoutée | Attribution et trois avertissements de fin d'escalier |
| Basher | Sape horizontale 32 images par sens | Attribution et refus sur acier/sens interdit |
| Miner | Minage diagonal 24 images par sens | Attribution et refus sur acier/sens interdit |
| Digger | Creusement vertical 16 images, indépendant du sens | Attribution et refus sur acier |

Les effets utilisent désormais les identifiants **et les notes** des appels du lecteur Amiga. Onze effets sont exportés dans `audio/gameplay_*.wav`, en complément des 111 échantillons bruts. Les manifestes `gameplay-audio.json` et `animations.json` documentent les associations. Le sauvetage est à présent résolu depuis l'appel `0x0c1b` : sample 27, `BasicFX_007`, note 12. Cela remplace l'association erronée de la version 0.1.1. L'explosion utilise le sample 29 (`BasicFX_009`), l'attribution le sample 23 (`BasicFX_003`).

Les sons sont mis en file puis consommés une fois : les attributions pendant une pause restent audibles et les pas de simulation accélérés ne perdent pas les événements. Les séquences de sauvetage, écrasement, noyade et combustion ont également été ajoutées avec leurs effets distincts.

Limites : les identifiants, plans, masques, dimensions et hauteurs audio sont contrôlés contre les données originales ; cela ne rend pas l'ensemble de la physique identique à l'Amiga. La séquence d'explosion affiche le flash d'origine, mais ne simule pas encore toutes les particules originales. Les interactions et certains rythmes restent ceux du moteur expérimental.

## Historique 0.1.1 — corrections audio et Digger (supplantées par 0.1.2)

- « Let's go » utilise `BasicFX` à l'offset `0x20b0` (export `BasicFX_002.wav`) et ne joue qu'au premier lemming, au lieu de rejouer un son de l'introduction à chaque apparition.
- Le sauvetage utilisait `BasicFX_010.wav` : cette association a été corrigée en 0.1.2 après résolution des identifiants dans le lecteur Amiga.
- Le Digger utilise la séquence originale à l'adresse Amiga `0xd55e` dans `Code` : 16 images de 16 × 14 pixels, trois plans couleur et un masque, sans variante gauche/droite. L'ancienne séquence à `0x13936` était une autre action.
- La capture CPU et le rendu à l'écran utilisent le même cycle corrigé. Le contrôle `verify` compare les deux WAV et les 16 images aux données du disque. Les dix tests passent, dont la vérification que la voix de départ ne se répète pas.

## Jouer sur Mac

Ouvrir `dist/Lemmings Rust.app` ou l'application livrée avec le compte rendu. La version produite ici est pour Apple Silicon, signée localement (sans notarisation). Elle ne nécessite ni émulateur Amiga ni ROM Kickstart.

Depuis les sources :

```sh
cd /Users/ptro/Projets/Lemmings-1
cargo run --release -- play
```

Choisir une difficulté puis un niveau. Les 120 niveaux de la campagne sont accessibles pour faciliter les essais. Les aperçus et niveaux proviennent des données Amiga, y compris les variantes de `oddtable`.

- Clic sur une compétence, puis sur un lemming : attribution. Les touches 1 à 8 sélectionnent les compétences dans l'ordre du panneau.
- Espace : pause ; F : accélération ×4 ; R : recommencer ; Échap : sélection des niveaux.
- Flèches gauche/droite, bords de l'aire de jeu ou clic sur la minicarte : défilement.
- RATE −/+ : fréquence de sortie, limitée au minimum du niveau et au maximum 99.
- NUKE : décompte d'explosion de tous les lemmings présents.
- F12 : export CPU de l'aire de jeu (sans interface) dans le fichier temporaire `lemmings-rust.png` ; évite la capture GPU non prise en charge par le backend Metal.

La progression est enregistrée dans `~/Library/Application Support/LemmingsRust/progress.json` sur macOS, `LOCALAPPDATA/LemmingsRust` sur Windows et `XDG_DATA_HOME/lemmings-rust` (ou `~/.local/share/lemmings-rust`) sur Linux.

## Extraction reproductible

Rust récent (édition 2024 ; version utilisée : 1.99.1). Le premier lancement de Cargo télécharge les dépendances. Les images originales sont lues uniquement, sans modification.

```sh
# La destination doit être nouvelle : aucun écrasement implicite.
cargo run --release -- import . assets-new
# Rendre les partitions avec le lecteur musical extrait du disque.
cargo run --release -- music assets-new 600
# Contrôler l'ensemble des ressources et résoudre automatiquement « Just dig! ».
cargo run --release -- verify assets-new
```

La limite de 600 secondes sert uniquement de garde-fou pendant la recherche d'une boucle musicale. Le rendu s'arrête dès qu'un état complet du lecteur se répète. Le catalogue indique la durée et si la boucle a été détectée. Pour ne rendre qu'une piste : `cargo run --release -- music assets-new 600 18`.

`cargo run --release -- rebuild assets` réexporte les sprites et les niveaux à partir des fichiers déjà extraits. Ne recalcule pas les tables des styles ni les musiques.

## Ressources obtenues

- 156 fichiers nommés extraits du répertoire spécifique au jeu, à partir du premier cylindre de données (`0x1600`).
- Décompression inverse avec contrôle XOR pour les ressources compressées ; les octets bruts des fichiers non compressés sont conservés.
- 5 styles de terrain : tuiles à 3 plans de couleur + masque, objets à 4 plans + masque, palettes et dimensions issues de `leveldata`.
- 120 niveaux de campagne reconstruits en 1600 × 160 pixels. Les quatre décors spéciaux sont décodés depuis leurs ILBM.
- Sprites natifs de marche, chute, escalade, parachute, blocage, construction, creusement, sape et minage, et séquence d'écrasement ; données récupérées dans `Code`.
- 14 images IFF autonomes et 111 sons/instruments 8SVX convertis en WAV.
- 21 musiques rendues en WAV PCM mono 16 bits / 22 050 Hz ; boucle détectée pour chacune.

Organisation dans `assets/` : `raw/` fichiers extraits/décompressés, `images/` ILBM convertis, `audio/` sons/instruments WAV, `styles/` tuiles/animations, `sprites/` animations des lemmings, `levels/` terrains, `music/` partitions rendues. `manifest.json` relie chaque fichier à son disque, son offset et ses tailles. `source-disks.json` contient les SHA-256 des deux disques utilisés.

## Ce qui reste à faire

La physique est une nouvelle implémentation à pas fixe (17 Hz), pas une traduction instruction par instruction du moteur Amiga. Les huit compétences ont un comportement jouable, mais leurs cadences, masques de destruction, transitions, interactions et règles de sélection ne sont pas encore entièrement fidèles. La résolution des 120 niveaux n'est donc pas garantie. Le test automatisé de référence valide le premier tutoriel (10 lemmings sauvés), pas toute la campagne.

Les pièges, liquides et zones à sens unique utilisent les tables originales, avec une gestion simplifiée de leurs interactions et animations. Les objets n'ont pas encore tous leurs modes de superposition d'origine. La musique emploie le vrai lecteur de partitions, mais la synthèse Paula reste simplifiée : mono, sans filtre analogique, ni émulation matérielle au cycle près. Les événements sonores des compétences sont associés aux appels originaux, avec les limites de synchronisation du moteur expérimental.

Les 20 terrains supplémentaires sont conservés dans les blocs de niveaux bruts ; le mode deux joueurs n'est pas implémenté. L'introduction ANIM et ses deltas sont conservés dans les fichiers extraits, mais leur lecture n'est pas intégrée. Les écrans de fin et le système de mots de passe d'origine ne sont pas reproduits.

## Architecture et validation

- `src/assets.rs` : répertoire des ADF, décompression, IFF ILBM / 8SVX, plans et export des styles/sprites.
- `src/level.rs` : ordre des niveaux, variantes, composition des terrains et placements.
- `src/music.rs` : exécution bornée du lecteur musical original dans l'interpréteur Rust `m68000`, synthèse des quatre canaux Paula et recherche des boucles.
- `src/presentation.rs` : associations vérifiées des animations et sons.
- `src/sim.rs` : simulation indépendante de l'affichage, terrain destructible, acier, lemmings et compétences.
- `src/ui.rs` : sélection, rendu, souris/clavier, sons, musique et progression.
- `src/verify.rs` : vérification des ressources et scénario complet du tutoriel sans fenêtre.

```sh
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo run --release -- verify assets
./scripts/bundle-macos.sh
```

Mac : Metal. Windows/Linux : backend natif fourni par Macroquad. L'exécutable Apple Silicon a été compilé et lancé sur ce Mac ; les autres plateformes n'ont pas été compilées ou testées. Linux requiert les bibliothèques système du backend graphique et ALSA pour l'audio.

## Références

- Format de disquette standard, consulté pour distinguer ce répertoire du système AmigaDOS : https://adflib.github.io/FAQ/adf_info.html
- Description/implémentation des variantes ByteKiller : https://github.com/temisu/ancient/blob/master/src/ByteKillerDecompressor.cpp
- Interpréteur 68000 Rust : https://docs.rs/m68000/0.2.3/m68000/ (MPL-2.0).
- Bibliothèque de fenêtre, rendu et audio : https://github.com/not-fl3/macroquad (MIT/Apache-2.0).

Les ressources originales restent celles de Lemmings (DMA Design / Psygnosis). L'archive des sources ne contient pas les ADF ni les ressources du jeu ; l'import utilise les disques locaux de l'utilisateur.
