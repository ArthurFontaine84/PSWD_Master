package fr.arthurfontaine.pswdmaster

import android.app.Activity
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.os.PersistableBundle
import android.webkit.WebView
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.Plugin
import java.util.UUID

@InvokeArg
class CopySecretArgs {
  lateinit var value: String
  var clearAfterMs: Long = 20_000
}

/**
 * Presse-papiers sensible (SPEC §9, §12) : contenu marqué sensible, effacé après le délai s'il
 * contient encore la copie. Chaque copie porte un identifiant aléatoire, gardé avec son échéance
 * dans les préférences : une copie survit ainsi à l'arrêt de l'application et est effacée au
 * lancement suivant. Rien de dérivé du mot de passe n'est enregistré.
 */
@TauriPlugin
class SecureClipboardPlugin(private val activity: Activity) : Plugin(activity) {
  private val clipboard = activity.getSystemService(ClipboardManager::class.java)
  private val pending = activity.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
  private val timer = Handler(Looper.getMainLooper())

  override fun load(webView: WebView) {
    if (pending.contains(KEY_ID)) {
      // Le focus de la fenêtre arrive juste après le lancement.
      val remaining = pending.getLong(KEY_DUE, 0) - System.currentTimeMillis()
      timer.postDelayed({ clearIfUnchanged() }, maxOf(remaining, FOCUS_DELAY_MS))
    }
  }

  @Command
  fun copySecret(invoke: Invoke) {
    val args = invoke.parseArgs(CopySecretArgs::class.java)
    activity.runOnUiThread {
      val id = UUID.randomUUID().toString()
      val clip = ClipData.newPlainText("", args.value)
      clip.description.extras = PersistableBundle().apply {
        // Android 13+ : contenu masqué dans l'aperçu et exclu des suggestions du clavier.
        putBoolean("android.content.extra.IS_SENSITIVE", true)
        putString(EXTRA_COPY_ID, id)
      }
      clipboard.setPrimaryClip(clip)
      pending.edit()
        .putString(KEY_ID, id)
        .putLong(KEY_DUE, System.currentTimeMillis() + args.clearAfterMs)
        .apply()
      // Seul le minuteur de la dernière copie efface.
      timer.removeCallbacksAndMessages(null)
      timer.postDelayed({ clearIfUnchanged() }, args.clearAfterMs)
      invoke.resolve()
    }
  }

  /**
   * Depuis Android 10, seule l'application qui a le focus peut lire le presse-papiers. Sans lui,
   * l'effacement se fait sans vérifier, quitte à effacer une copie faite ailleurs entre-temps.
   */
  private fun clearIfUnchanged() {
    val expected = pending.getString(KEY_ID, null) ?: return
    pending.edit().clear().apply()
    val readable = Build.VERSION.SDK_INT < Build.VERSION_CODES.Q || activity.hasWindowFocus()
    if (!readable || copyId() == expected) {
      clear()
    }
  }

  private fun copyId(): String? =
    clipboard.primaryClipDescription?.extras?.getString(EXTRA_COPY_ID)

  private fun clear() {
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
      clipboard.clearPrimaryClip()
    } else {
      clipboard.setPrimaryClip(ClipData.newPlainText("", ""))
    }
  }

  private companion object {
    const val PREFS = "secure-clipboard"
    const val KEY_ID = "copy-id"
    const val KEY_DUE = "due-at"
    const val EXTRA_COPY_ID = "fr.arthurfontaine.pswdmaster.COPY_ID"
    const val FOCUS_DELAY_MS = 500L
  }
}
