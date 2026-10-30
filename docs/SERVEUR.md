# Installer le serveur maison `pswd-server`

Le serveur synchronise vos coffres entre vos appareils et permet les coffres communs. Il ne garde que des coffres chiffrés : il ne connaît aucun mot de passe maître. Cible : **Ubuntu Server 26.04 LTS** (x86_64), d'abord dans une machine virtuelle pour essayer, puis sur un vrai PC. Conception détaillée : `docs/SPEC.md` § 11.

## 1. Construire le paquet `.deb`

Sur la machine Ubuntu elle-même (le paquet se construit sous Linux) :

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

## 4. Accès depuis l'extérieur

Par défaut, le serveur n'est joignable que sur le réseau de la maison. Pour l'atteindre ailleurs :

1. Donnez au serveur une adresse fixe sur le réseau local (réservation DHCP dans la box).
2. Dans la box, redirigez un port externe (par exemple 8443) vers le port 8443 de ce serveur.
3. Donnez un nom à votre connexion, de préférence un nom DNS dynamique (par exemple DuckDNS) : l'adresse IP de la box peut changer.
4. Dans PSWD Master : Administration → Serveur → « Adresse publique », par exemple `maison.duckdns.org:8443`. Les appareils la reçoivent au prochain échange, et l'essaient quand l'adresse locale ne répond pas.

Le certificat est vérifié quelle que soit l'adresse : un autre serveur à la même adresse serait refusé. Un VPN (WireGuard, Tailscale) est une alternative à la redirection de port.

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
