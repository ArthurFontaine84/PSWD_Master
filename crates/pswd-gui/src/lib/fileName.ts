/**
 * Le nom d'un fichier `.pswd` choisi, sans son dossier, ou `null` si l'adresse ne le donne pas : sur
 * Android, une adresse `content://` n'est souvent qu'un identifiant.
 */
export function displayedFileName(path: string): string | null {
  const decoded = path.startsWith("content:") ? decodeURIComponent(path) : path;
  const name = decoded.replace(/^.*[/\\:]/, "");
  return name.endsWith(".pswd") ? name : null;
}
