# pswd / PSWD Master

Gestionnaire de mots de passe personnel, local-first :

- **PSWD Master**, l'application graphique pour Windows, Linux et macOS et Android (iOS à venir) ;
- **`pswd`**, la même chose en ligne de commande.

Les coffres sont chiffrés localement (Argon2id + XChaCha20-Poly1305) et restent sur vos appareils : aucun service tiers n'intervient. Sur un ordinateur, l'application et la ligne de commande voient les mêmes coffres.

> 🚧 En cours de développement. Conception et feuille de route : [`docs/SPEC.md`](docs/SPEC.md).

Installation, premier coffre et sauvegardes : [`docs/INSTALLATION.md`](docs/INSTALLATION.md).

## PSWD Master

Créer un coffre, le déverrouiller, puis ajouter, chercher, copier et modifier ses entrées. Le menu « Gérer le coffre » change le mot de passe maître, importe un CSV ou supprime le coffre ; la liste des coffres propose la sauvegarde et la restauration (fichier `.pswd`, coffres au choix) ; les réglages choisissent le thème.

## Ligne de commande

```sh
pswd init perso                 # crée le coffre « perso »
pswd open perso                 # déverrouille et ouvre le shell interactif
```

Dans le shell : `list`, `search`, `show`, `copy`, `add`, `edit`, `rm`, `history`, `tags`, `gen`, `lock`, `quit`.

Autres commandes : `pswd passwd <nom>`, `pswd recover <nom>`, `pswd import <nom> <fichier.csv>`, `pswd backup <dossier>` (fichier `.pswd` sur une clé USB), `pswd restore <fichier.pswd>`, `pswd delete <nom>`.

## ⚠ À savoir

- **Mot de passe maître et clé de récupération perdus = coffre perdu.** Aucune récupération n'est possible. La clé de récupération n'est affichée qu'une fois, à la création : imprimez-la et rangez-la en lieu sûr.
- **Changer le mot de passe maître ne protège pas les anciennes copies** : l'ancien mot de passe ouvre toujours les copies antérieures du coffre (sauvegardes). Si votre mot de passe a fuité, cela ne suffit pas.
- **Presse-papiers sous Linux** : l'exclusion de l'historique dépend de votre gestionnaire de presse-papiers ; certains conservent quand même une copie.
- **Fichiers CSV importés** : ils contiennent tout en clair. Supprimez-les dès que possible après l'import.
