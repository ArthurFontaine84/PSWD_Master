# Guide d'utilisation de PSWD Master

1. [L'essentiel en une minute](#1-lessentiel-en-une-minute)
2. [Quel usage choisir ?](#2-quel-usage-choisir-)
3. [Utiliser PSWD Master sur un appareil (local)](#3-utiliser-pswd-master-sur-un-appareil-local)
4. [Utiliser PSWD Master avec un serveur](#4-utiliser-pswd-master-avec-un-serveur)
5. [Installer le serveur](#5-installer-le-serveur)
6. [Bon à savoir](#6-bon-à-savoir)
7. [En cas de problème](#7-en-cas-de-problème)
8. [Installer l'application sur un autre appareil](#8-installer-lapplication-sur-un-autre-appareil)

## 1. L'essentiel en une minute

- Un **coffre** contient vos **entrées** : des mots de passe (avec identifiant et adresse du site) ou des notes sécurisées (code Wi-Fi, code d'une alarme…). Vous pouvez avoir plusieurs coffres : « Perso », « Travail », « Famille »…
- Chaque coffre a son **mot de passe maître**. C'est le seul mot de passe à retenir.
- À la création, une **clé de récupération** s'affiche une seule fois. Elle remplace le mot de passe maître en cas d'oubli : notez-la sur papier.

## 2. Quel usage choisir ?

| Votre situation | Usage conseillé |
|---|---|
| J'utilise un seul ordinateur ou un seul téléphone | **Local** (§ 3) : rien d'autre à installer |
| Je veux passer mes mots de passe d'un appareil à un autre, de temps en temps | **Local**, avec un fichier de sauvegarde (§ 3.6) |
| Je veux les mêmes coffres, toujours à jour, sur mon PC et mon téléphone | **Serveur** (§ 4) |
| Je veux partager certains mots de passe avec ma famille | **Serveur**, avec un coffre commun (§ 4.4) |

Tout ce qui est décrit au § 3 fonctionne aussi avec un serveur.

## 3. Utiliser PSWD Master sur un appareil (local)

### 3.1 Créer un coffre

1. Sur l'écran d'accueil, touchez **+** (Nouveau coffre) en haut.
2. Donnez un nom au coffre et choisissez un mot de passe maître conforme. Laissez **Coffre commun** décoché (il sert au partage, § 4.4).
3. La clé de récupération s'affiche : recopiez-la, rangez-la à en lieu sûr.

### 3.2 Ouvrir et verrouiller

L'écran d'accueil liste vos coffres. Touchez-en un et saisissez son mot de passe maître. **Verrouiller** le referme.

### 3.3 Ajouter une entrée

1. Dans le coffre ouvert : **Nouvelle entrée**.
2. Choisissez le type : **Mot de passe** ou **Note sécurisée**.
3. Remplissez le titre, l'identifiant, les adresses web, les notes et les tags.
4. Pour le mot de passe : **Saisir** le vôtre, ou **Générer** un mot de passe solide.

Les **tags** classent vos entrées (« banque », « jeux »…)

### 3.4 Retrouver et copier un mot de passe

- La **recherche** porte sur le titre, l'identifiant, les adresses et les tags (`Ctrl+F` sur ordinateur). Le menu déroulant **Tous les tags** filtre par tag.
- Touchez une entrée pour voir son détail. **Copier** met le mot de passe dans le presse-papiers ; il en est effacé après 20 secondes. **Afficher** le montre pendant 30 secondes.
- Une adresse web s'ouvre dans votre navigateur d'un simple clic.

### 3.5 Modifier, consulter l'historique, supprimer

Dans le détail d'une entrée : **Modifier**, ou **Supprimer** (une confirmation est demandée). L'**historique** garde les 10 versions précédentes de l'entrée (date, titre, identifiant, « mot de passe différent »), sans jamais montrer les anciens mots de passe.

### 3.6 Sauvegarder et restaurer

PSWD Master offre la possibilité de sauvegarder les coffres dans un format de fichier personalisé.

Depuis l'écran d'accueil :

- **Sauvegarder** : cochez les coffres voulus, puis choisissez où enregistrer le fichier `.pswd` (une clé USB, un disque dur, etc...). Les coffres y restent chiffrés.
- **Restaurer** : choisissez un fichier `.pswd`, cochez les coffres à récupérer. Chacun est ajouté comme nouveau coffre : un coffre existant n'est jamais remplacé (en cas de nom déjà pris, il est renommé). Il s'ouvre avec le mot de passe maître qu'il avait au moment de la sauvegarde.

Ce même fichier sert à **passer vos coffres sur un autre appareil** : sauvegardez sur l'un, restaurez sur l'autre.

### 3.7 Gérer un coffre

Dans le coffre ouvert, le menu **Gérer le coffre** (en haut) propose :

- **Changer le mot de passe maître** (l'actuel est demandé) ;
- **Importer un CSV** : pour reprendre vos mots de passe d'un autre gestionnaire (l'écran d'import propose **Télécharger le modèle**, un fichier à remplir dans un tableur) ;
- **Supprimer ce coffre** (son nom est à retaper) : faites une sauvegarde avant.

**Format du fichier CSV**, à préparer dans Excel puis à enregistrer en « CSV (séparateur : point-virgule) » :

| Titre / Title | Type / Kind | Nom d'utilisateur / Username | Mot de passe / Password | Urls | Notes | Tags |
|---|---|---|---|---|---|---|
| GitHub | login | arthur | xxxx | `https://github.com` | | dev |
| Code Wi-Fi | note | | | | Box : ABCD-1234 | maison |

- La première ligne doit être exactement celle du tableau.
- Type : `login` pour un mot de passe, `note` pour une note sécurisée.
- Plusieurs urls ou plusieurs tags : séparez-les par `;` dans la même cellule.
- Si une ligne est incorrecte, rien n'est importé et son numéro est indiqué.
- Ce fichier contient vos mots de passe en clair : **supprimez-le et vider la corbeille**, dès l'import terminé.

**Mot de passe maître oublié ?** Sur l'écran de déverrouillage, touchez **Mot de passe oublié ?**, saisissez la clé de récupération et choisissez un nouveau mot de passe.

### 3.8 Réglages

L'icône d'engrenage, en haut de l'accueil, ouvre les **Réglages** : thème clair, sombre ou comme le système.

## 4. Utiliser PSWD Master avec un serveur

Le serveur est un petit programme installé sur un PC de la maison, toujours allumé (§ 5). Il garde une copie **chiffrée** de vos coffres et la transmet à vos appareils. Il ne connaît aucun mot de passe maître : il ne peut pas lire vos coffres.

Chaque personne a un **compte** sur le serveur, et chaque appareil y est connecté avec une **clé de connexion** (13 groupes de 4 caractères). Une clé ne sert qu'une fois et expire au bout de 7 jours.

### 4.1 Connecter un appareil

1. Sur l'écran d'accueil, touchez l'icône **Serveur** en haut.
2. Saisissez l'adresse du serveur (par exemple `192.168.1.10`), la clé de connexion et un nom pour cet appareil (« PC du salon », « Téléphone de Léa »…).
3. **Nom du compte** : seulement pour le tout premier appareil, avec la clé administrateur affichée à l'installation du serveur. Sinon, laissez vide.
4. **Se connecter**.

À la connexion, tous les coffres de l'appareil sont envoyés au serveur (chiffrés), et ceux de votre compte sont téléchargés.

**Ajouter un autre de vos appareils** : sur un appareil déjà connecté, écran **Serveur** → **Ajouter un appareil**. Une clé s'affiche : saisissez-la sur le nouvel appareil, comme ci-dessus.

**Ajouter un membre de la famille** : voir § 4.6.

### 4.2 La synchronisation

Elle est **automatique** : tant que l'application est ouverte, les coffres s'échangent avec le serveur toutes les 5 secondes. Une modification faite sur le téléphone apparaît sur le PC en quelques secondes. Sans réseau, vous travaillez normalement : tout est envoyé au retour de la connexion. Le bouton **Synchroniser** lance un échange tout de suite et en affiche le compte rendu.

Une **pastille** de couleur, sur chaque coffre, indique son état (touchez-la pour lire son explication) :

| Pastille | Signification |
|---|---|
| 🟢 Verte | Synchronisé : identique au serveur |
| 🟠 Orange | Différent du serveur : sera mis à jour au prochain échange. Si elle reste orange, le coffre a été modifié ici et ailleurs : ouvrez-le pour finir la synchro |
| 🔴 Rouge | Pas encore sur le serveur, absent de votre compte, ou serveur injoignable |

### 4.3 Supprimer un coffre synchronisé

**Gérer le coffre** → **Supprimer ce coffre**, puis au choix :

- **Supprimer partout** : effacé du serveur et de tous vos appareils ;
- **Retirer de cet appareil** : le coffre reste sur le serveur et vos autres appareils. Pour le récupérer : écran **Serveur** → **Coffres retirés de cet appareil** → **Télécharger**.

### 4.4 Partager un coffre avec d'autres personnes

Un **coffre commun** se partage entre plusieurs comptes du serveur. Tous ses membres l'ouvrent avec le même mot de passe maître.

1. **Créer** : nouveau coffre, case **Coffre commun** cochée (ce choix est définitif).
2. **Inviter** : ouvrez le coffre → **Gérer le coffre** → **Membres** → **Inviter un compte**.
3. **Accepter** : l'invitation apparaît en haut de l'accueil de la personne invitée, qui touche **Accepter**. Le coffre est téléchargé chez elle.
4. **Transmettre le mot de passe** du coffre de vive voix ou sur papier, jamais par le serveur.

Pour partir : **Gérer le coffre** → **Quitter ce coffre**. Le dernier membre qui le quitte le supprime. Après le départ de quelqu'un, changez le mot de passe du coffre : l'ancien membre le connaît.

### 4.5 Se déconnecter

Écran **Serveur** → **Se déconnecter**. Laissez cochée la case **Garder une copie des coffres sur cet appareil** pour continuer à les utiliser en local.

### 4.6 Administrer le serveur

Le compte créé avec la clé administrateur est **administrateur**. Le panneau s'ouvre par **Réglages** → **Administration**, en quatre onglets :

| Onglet | Ce qu'on y fait |
|---|---|
| **Comptes** | **Ajouter un membre** (une clé s'affiche : transmettez-la avec l'adresse du serveur), voir les appareils de chacun, **révoquer** un appareil perdu ou volé, nommer un autre administrateur, supprimer un compte |
| **Coffres** | Voir les membres de chaque coffre, **restaurer une version** (le serveur garde les 15 dernières), supprimer un coffre du serveur |
| **Clés** | Voir les clés de connexion pas encore utilisées, les annuler |
| **Serveur** | Adresse publique (grisée : arrivera dans une prochaine version, § 5.3), sauvegarde du serveur (§ 5.4) |

**Restaurer une version** sert à rattraper une erreur : une entrée supprimée par mégarde, un mot de passe de coffre commun oublié… La version choisie devient un nouveau coffre (`<nom>-v<numéro>`), avec les mêmes membres ; le coffre actuel ne change pas.

## 5. Installer le serveur

### 5.1 Ce qu'il faut

- Un PC toujours allumé (un vieux PC suffit), avec un processeur Intel ou AMD 64 bits, sous **Debian, Ubuntu ou une distribution qui en dérive** (testé sur Ubuntu Server 26.04) ;
- le fichier d'installation du serveur, `pswd-server_<version>_amd64.deb`, fourni avec PSWD Master.

### 5.2 Installation

1. **Envoyez le fichier sur le serveur.** Depuis votre ordinateur (Windows, macOS ou Linux), dans un terminal ouvert dans le dossier du fichier, en remplaçant `utilisateur` et l'adresse par ceux de votre serveur :

   ```sh
   scp pswd-server_*_amd64.deb utilisateur@192.168.1.10:~
   ```

   Le fichier arrive dans votre dossier personnel sur le serveur.

2. **Installez-le.** Sur le serveur :

   ```sh
   cd ~
   sudo apt install ./pswd-server_*_amd64.deb
   ```

L'installation affiche **l'adresse du serveur** et une **clé administrateur** : recopiez-les et connectez votre premier appareil (§ 4.1, en remplissant « Nom du compte »). Le serveur démarre tout seul, et à chaque démarrage du PC.

Le serveur écoute sur le port TCP **8443**. Si un pare-feu filtre les connexions entrantes, autorisez ce port selon vos règles habituelles, de préférence depuis votre réseau local seulement.

**Choisir un autre port** : faites-le **avant de connecter le premier appareil**, car chaque appareil retient le port à sa connexion. Modifiez la ligne `port = 8443` de `/etc/pswd-server/config.toml`, puis redémarrez le serveur :

```sh
sudo systemctl restart pswd-server
```

Dans l'application, saisissez alors l'adresse avec le port, par exemple `192.168.1.10:42443` (l'adresse affichée à l'installation indique encore 8443).

### 5.3 Accéder au serveur hors de chez soi (prochaine version)

Pas encore disponible : l'accès au serveur depuis l'extérieur de la maison arrivera dans une prochaine version. Pour l'instant, vos appareils se synchronisent quand ils sont connectés au réseau de la maison ; ailleurs, ils fonctionnent normalement et rattrapent les modifications à leur retour.


### 5.4 Sauvegarder le serveur

**Administration** → **Serveur** → icône de sauvegarde, ou sur le PC du serveur : `sudo pswd-server backup /media/cle-usb`. Le fichier contient les comptes, les coffres chiffrés et le certificat du serveur : rangez-le hors de cette machine.

### 5.5 Au quotidien

Sur le PC du serveur :

| Besoin | Commande |
|---|---|
| Le serveur fonctionne-t-il ? | `systemctl status pswd-server` |
| Clé administrateur perdue ou expirée | `sudo -u pswd-server pswd-server reset-admin` |
| Mettre à jour | Installez le nouveau fichier de la même façon : `sudo apt install ./pswd-server_<nouvelle version>_amd64.deb` (les données sont gardées) |
| Restaurer une sauvegarde | Voir § 5.6 |

### 5.6 Restaurer une sauvegarde du serveur

La restauration remplace tout le contenu du serveur. Elle n'est donc acceptée que sur un serveur **neuf** : aucun compte, aucun coffre. Le serveur restauré garde son identité : les appareils se reconnectent seuls, sans nouvelle clé.

1. **Repartir d'un serveur neuf.** Sur une nouvelle machine, installez simplement le serveur (§ 5.2). Sur une machine où il a déjà servi, effacez-le d'abord avec ses données, puis réinstallez-le :

   ```sh
   sudo apt purge pswd-server
   sudo apt install ./pswd-server_*_amd64.deb
   ```

   **N'utilisez pas la clé administrateur affichée** : elle créerait un compte, et le serveur ne serait plus neuf.

2. **Restaurer**, le serveur arrêté (l'installation l'a démarré) :

   ```sh
   sudo systemctl stop pswd-server
   sudo pswd-server restore <fichier de sauvegarde>
   sudo systemctl start pswd-server
   ```

## 6. Bon à savoir

- **Changer le mot de passe maître ne protège pas les anciennes copies** : l'ancien mot de passe ouvre toujours les sauvegardes faites avant le changement.
- **Presse-papiers** : sous Linux, et avec certains claviers de téléphone qui gardent un historique, une copie peut rester dans l'historique du presse-papiers.
- **Sur téléphone**, l'application n'apparaît pas dans les captures d'écran ni dans l'aperçu des applications récentes.

## 7. En cas de problème

| Problème | Solution |
|---|---|
| Mot de passe maître oublié | **Mot de passe oublié ?** sur l'écran de déverrouillage, avec la clé de récupération |
| Mot de passe et clé de récupération perdus | Le coffre ne peut pas être ouvert. Restaurez une sauvegarde dont vous connaissez le mot de passe |
| Mot de passe changé sur un autre appareil, refusé ici | Attendez quelques secondes que la synchro arrive, puis réessayez |
| Pastille rouge sur tous les coffres | Le serveur ne répond pas : vérifiez le réseau, ou que le PC du serveur est allumé. Vos coffres restent utilisables |
| Pastille orange qui reste | Ouvrez le coffre : la synchro se termine à l'ouverture |
| Clé de connexion refusée | Elle a déjà servi ou a expiré (7 jours) : demandez-en une nouvelle |
| Windows ou macOS bloque l'installation | L'installateur n'est pas signé : « Informations complémentaires » → « Exécuter quand même » (Windows) ; clic droit → « Ouvrir » (macOS) |

## 8. Installer l'application sur un autre appareil

| Appareil | Fichier | Installation |
|---|---|---|
| Windows | `PSWD Master_<version>_x64-setup.exe` | Double-cliquez. Aucun droit d'administrateur n'est demandé |
| Linux (Debian, Ubuntu…) | `PSWD Master_<version>_amd64.deb` | `sudo apt install ./PSWD*.deb`, ou double-clic selon la distribution |
| Autre Linux | `PSWD Master_<version>_amd64.AppImage` | Rendez-le exécutable (`chmod +x PSWD*.AppImage`), puis lancez-le |
| macOS | `PSWD Master_<version>_<processeur>.dmg` | Ouvrez-le et glissez l'application dans « Applications » |
| Android | `PSWD Master_<version>.apk` | Ouvrez le fichier sur le téléphone et autorisez l'installation d'applications de cette source |
| iPhone, iPad | `PSWD Master_<version>.ipa` | Pas encore disponible |

Pour mettre à jour, installez la nouvelle version par-dessus l'ancienne : vos coffres sont conservés. Ne désinstallez pas l'application sur Android sans avoir fait une sauvegarde : la désinstallation efface ses coffres.
