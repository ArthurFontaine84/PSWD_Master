# pswd

Gestionnaire de mots de passe personnel en ligne de commande (Windows, Linux).
Le coffre est chiffré localement (Argon2id + XChaCha20-Poly1305) et reste sur vos appareils : aucun service tiers n'intervient.

> 🚧 En cours de développement. Conception et feuille de route : [`docs/SPEC.md`](docs/SPEC.md).

## Démarrage

```sh
pswd init perso                 # crée le coffre « perso »
pswd open perso                 # déverrouille et ouvre le shell interactif
```

Dans le shell : `list`, `search`, `show`, `copy`, `add`, `edit`, `rm`, `history`, `tags`, `gen`, `lock`, `quit`.

Autres commandes : `pswd passwd <nom>`, `pswd recover <nom>`, `pswd import <nom> <fichier.csv>`, `pswd export <nom> <fichier.csv>`, `pswd backup <dossier>` (clé USB), `pswd restore <sauvegarde>`, `pswd delete <nom>`.

## ⚠ À savoir

- **Mot de passe maître et clé de récupération perdus = coffre perdu.** Aucune récupération n'est possible. La clé de récupération n'est affichée qu'une fois, à `init` : imprimez-la et rangez-la en lieu sûr.
- **Changer le mot de passe maître ne protège pas les anciennes copies** : l'ancien mot de passe ouvre toujours les copies antérieures du coffre (sauvegardes). Si votre mot de passe a fuité, cela ne suffit pas.
- **Presse-papiers sous Linux** : l'exclusion de l'historique dépend de votre gestionnaire de presse-papiers ; certains conservent quand même une copie.
- **Fichiers CSV** (import et export) : ils contiennent tout en clair. Supprimez-les dès que possible.
