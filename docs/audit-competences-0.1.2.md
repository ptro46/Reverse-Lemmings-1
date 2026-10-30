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

