# pswd

Gestionnaire de mots de passe personnel en ligne de commande (Windows, Linux).
Le coffre est chiffré localement (Argon2id + XChaCha20-Poly1305), lisible hors ligne, et synchronisé via un dépôt Git privé qui ne voit que des données chiffrées.

> 🚧 En cours de développement. Conception : [`docs/SPEC.md`](docs/SPEC.md).

## Démarrage

```sh
pswd init perso                 # premier appareil : crée le coffre « perso »
pswd join perso <url-du-dépôt>  # autre appareil : rejoint le coffre
pswd open perso                 # déverrouille et ouvre le shell interactif
```

Dans le shell : `list`, `search`, `show`, `copy`, `add`, `edit`, `rm`, `history`, `tags`, `gen`, `sync`, `lock`, `quit`.

Autres commandes : `pswd sync`, `pswd passwd`, `pswd recover`, `pswd import pass <fichier.csv>`, `pswd export <fichier.csv>`.

Le dépôt distant doit être **privé** et utilisé avec un jeton d'accès limité à ce seul dépôt.

## ⚠ À savoir

- **Mot de passe maître et clé de récupération perdus = coffre perdu.** Aucune récupération n'est possible. La clé de récupération n'est affichée qu'une fois, à `init` : imprimez-la et rangez-la en lieu sûr.
- **Changer le mot de passe maître ne protège pas l'historique** : l'ancien mot de passe ouvre toujours les anciennes versions du coffre conservées dans Git. Si votre mot de passe a fuité, cela ne suffit pas.
- **Presse-papiers sous Linux** : l'exclusion de l'historique dépend de votre gestionnaire de presse-papiers ; certains conservent quand même une copie.
- **Fichiers CSV** (import et export) : ils contiennent tout en clair. Supprimez-les dès que possible.
