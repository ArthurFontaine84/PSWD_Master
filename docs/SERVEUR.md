# Installer le serveur maison `pswd-server`

Le serveur synchronise vos coffres entre vos appareils et permet les coffres communs. Il ne garde que des coffres chiffrés : il ne connaît aucun mot de passe maître. Cible : **Ubuntu Server 26.04 LTS** (x86_64), d'abord dans une machine virtuelle pour essayer, puis sur un vrai PC. Pour un autre Linux ou une machine Windows : conteneur Docker (section 7). Conception détaillée : `docs/SPEC.md` § 11.

## 1. Construire le paquet `.deb`

Le paquet se construit sous Linux (Debian ou Ubuntu) : directement sur le serveur, ou sur un autre ordinateur dans WSL ou une machine virtuelle, puis le copier sur le serveur (`scp`). Un paquet construit sur une version d'Ubuntu s'installe sur la même version ou une plus récente.

```sh
sudo apt install build-essential git curl
curl https://sh.rustup.rs -sSf | sh        # Rust ; rouvrir le terminal ensuite
cargo install cargo-deb
git clone <adresse du dépôt> pswd && cd pswd
cargo deb -p pswd-server
```

Le paquet est rangé dans `target/debian/pswd-server_<version>_amd64.deb`.

## 2. Installer

```sh
sudo apt install ./target/debian/pswd-server_<version>_amd64.deb
```

L'installation, sans autre étape :

- installe `/usr/bin/pswd-server` et la configuration `/etc/pswd-server/config.toml` (port 8443) ;
- crée l'utilisateur système `pswd-server` (sans connexion : le serveur ne tourne jamais en `root`) et le dossier `/var/lib/pswd-server/`, lisible par lui seul ;
- crée la base et le certificat, puis **affiche l'adresse du serveur et une clé administrateur** ;
- installe et démarre le service, relancé à chaque démarrage de la machine.

Recopiez l'adresse et la clé. Dans PSWD Master, écran **Serveur** : saisissez l'adresse, la clé, le nom de l'appareil et votre nom (le nom du compte administrateur). La clé ne sert qu'une fois et expire dans 7 jours.

Si le pare-feu `ufw` est actif, ouvrez le port :

```sh
sudo ufw allow 8443/tcp
```

## 3. Ajouter des membres et des appareils

Tout se fait depuis PSWD Master :

- **un membre de la famille** : écran Serveur → Administration → Comptes → « Ajouter un membre ». Transmettez-lui l'adresse et la clé affichées ;
- **un autre de vos appareils** : écran Serveur → « Ajouter un appareil », puis connectez le nouvel appareil avec cette clé.

## 4. Accès depuis l'extérieur (prochaine version)

> **Pas encore disponible.** L'accès au serveur depuis l'extérieur de la maison n'est pas encore implémenté : il arrivera dans une prochaine version. En attendant, le serveur n'est joignable que sur le réseau de la maison, et les appareils se synchronisent quand ils y sont connectés.

## 5. Sauvegarder et restaurer

La base contient les comptes, les coffres chiffrés et le certificat du serveur (avec sa clé privée) : rangez les sauvegardes en lieu sûr, hors de cette machine.

```sh
sudo pswd-server backup /media/cle-usb             # sans arrêter le serveur
```

Le fichier `pswd-server-sauvegarde_<date>.db` est créé dans ce dossier, qui doit exister ; rien n'est écrasé. PSWD Master propose la même sauvegarde (Administration → Serveur).

Restaurer se fait sur un serveur **neuf** (sans compte ni coffre), par exemple après réinstallation sur un nouveau disque :

```sh
sudo apt install ./pswd-server_<version>_amd64.deb   # n'utilisez pas la clé affichée
sudo systemctl stop pswd-server
sudo pswd-server restore /media/cle-usb/pswd-server-sauvegarde_<date>.db
sudo systemctl start pswd-server
```

Le certificat restauré est le même : les appareils se reconnectent sans rien refaire. Un serveur qui a déjà des comptes ou des coffres n'est jamais remplacé.

## 6. Au quotidien

| Besoin | Commande |
|---|---|
| État du service | `systemctl status pswd-server` |
| Journal | `journalctl -u pswd-server` |
| Nouvelle clé administrateur (perdue ou expirée) | `sudo -u pswd-server pswd-server reset-admin` |
| Changer de port | modifier `/etc/pswd-server/config.toml`, puis `sudo systemctl restart pswd-server` |
| Mettre à jour | `git pull`, `cargo deb -p pswd-server`, puis `sudo apt install ./target/debian/…deb` : les données sont gardées |
| Désinstaller en gardant les données | `sudo apt remove pswd-server` |
| Tout effacer, données comprises | `sudo apt purge pswd-server` |

La nouvelle clé administrateur crée un **nouveau** compte administrateur à la connexion ; l'ancien compte reste, à supprimer depuis le panneau si besoin.

## 7. Variante : en conteneur Docker

À la place du paquet `.deb` (sections 1, 2, 5 et 6), le serveur peut tourner dans un conteneur Docker Le conteneur est joignable sur le port 8543 de la machine, et non 8443 comme le paquet (à modifier selon la configuration)

L'image est construite puis envoyée au serveur : le serveur n'a besoin ni du dépôt ni de Rust, seulement de Docker et du fichier `compose.yaml`.

### Ce qui protège la machine

Le moteur Docker tourne en `root`, mais le conteneur est bridé (`compose.yaml`) :

- le serveur y tourne sous un utilisateur ordinaire (numéro 10001), jamais en `root` ;
- tout est en lecture seule, sauf les données et les sauvegardes ;
- aucune capacité système (`cap_drop: ALL`) et aucun moyen d'en regagner (`no-new-privileges`) ;

Sous Linux, piloter Docker revient à être `root` : les commandes `docker` se lancent avec `sudo`, et on n'ajoute **jamais** son compte au groupe `docker` (ce serait donner les droits `root` sans mot de passe).

### Construire l'image (sur l'ordinateur où se trouve le dépôt)

```sh
docker build -f crates/pswd-server/docker/Dockerfile -t pswd-server .   # quelques minutes
docker save -o pswd-server.tar pswd-server
```

Copier ensuite `pswd-server.tar` et `crates/pswd-server/docker/compose.yaml` sur le serveur (`scp`, clé USB, dossier partagé), dans un même dossier.

`docker save -o` écrit le fichier lui-même.

### Installer Docker (sur le serveur)

Par exemple sous Ubuntu : `sudo apt install docker.io docker-compose-v2`

Toutes les commandes `docker` qui suivent se lancent dans le dossier de `compose.yaml` ; sous Linux, les précéder de `sudo`.

### Charger l'image et démarrer

```sh
docker load -i pswd-server.tar             # puis supprimer pswd-server.tar
docker compose run --rm pswd-server reset-admin
docker compose up -d
```

`reset-admin` crée la base, le certificat et affiche la clé administrateur, avant le premier démarrage. Le conteneur redémarre ensuite avec Docker (`restart: unless-stopped`).

La première commande `run` doit annoncer la création des volumes `pswd-server_donnees` et `pswd-server_sauvegardes`. 

Les données sont dans deux **volumes** Docker, `pswd-server_donnees` (base et certificat) et `pswd-server_sauvegardes`, rangés par Docker hors du conteneur : supprimer ou reconstruire le conteneur ne les touche pas, et ils appartiennent déjà au bon utilisateur, sans rien préparer.

Adresse du serveur : la première de `hostname -I` (Linux) ou l'« Adresse IPv4 » de `ipconfig` (Windows), port 8543. Le port se change dans `compose.yaml` (`"8543:8443"` : seul le nombre de gauche change).

### Pare-feu

- **Linux** : un port publié par Docker n'est **pas** filtré par `ufw` : il est ouvert dès le démarrage du conteneur, quoi que dise `ufw`.
- **Windows** : autoriser le port dans le pare-feu, dans un PowerShell lancé en administrateur :

  ```powershell
  New-NetFirewallRule -DisplayName "pswd-server" -Direction Inbound -Protocol TCP -LocalPort 8543 -Action Allow
  ```

Le serveur n'est joignable que depuis le réseau de la maison : l'accès depuis l'extérieur arrivera dans une prochaine version (section 4).

### Au quotidien

| Besoin | Commande |
|---|---|
| État | `docker compose ps` |
| Journal | `docker compose logs pswd-server` |
| Nouvelle clé administrateur | `docker compose exec pswd-server pswd-server reset-admin` |
| Sauvegarder | `docker compose exec pswd-server pswd-server backup /sauvegardes`, puis `docker compose cp pswd-server:/sauvegardes/<fichier>.db .` et ranger ce fichier hors de la machine |
| Mettre à jour | reconstruire et copier l'image (voir plus haut), `docker load -i pswd-server.tar`, puis `docker compose up -d` : les données sont gardées |
| Arrêter | `docker compose down` (les volumes, donc les données, restent) |
| Tout effacer, données comprises | `docker compose down -v --rmi all` |
