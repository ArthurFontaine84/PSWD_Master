package fr.arthurfontaine.pswdmaster

import android.app.Activity
import android.content.ClipData
import android.content.ClipboardManager
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.os.PersistableBundle
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.Plugin
import java.security.MessageDigest

@InvokeArg
class CopySecretArgs {
  lateinit var value: String
  var clearAfterMs: Long = 20_000
}

/**
 * Presse-papiers sensible (SPEC §9) : contenu marqué sensible, effacé après le délai s'il
 * contient encore la valeur copiée. Seule l'empreinte de cette valeur est gardée en mémoire.
 */
@TauriPlugin
class SecureClipboardPlugin(private val activity: Activity) : Plugin(activity) {
  private val clipboard = activity.getSystemService(ClipboardManager::class.java)
  private val timer = Handler(Looper.getMainLooper())
  private var copied: ByteArray? = null
  private var due = false

  @Command
  fun copySecret(invoke: Invoke) {
    val args = invoke.parseArgs(CopySecretArgs::class.java)
    activity.runOnUiThread {
      val clip = ClipData.newPlainText("", args.value)
      // Android 13+ : contenu masqué dans l'aperçu et exclu des suggestions du clavier.
      clip.description.extras = PersistableBundle().apply {
        putBoolean("android.content.extra.IS_SENSITIVE", true)
      }
      clipboard.setPrimaryClip(clip)
      copied = digest(args.value)
      due = false
      // Seul le minuteur de la dernière copie efface.
      timer.removeCallbacksAndMessages(null)
      timer.postDelayed({ clearIfUnchanged() }, args.clearAfterMs)
      invoke.resolve()
    }
  }

  override fun onResume() {
    if (due) {
      // Le focus de la fenêtre arrive juste après la reprise.
      timer.postDelayed({ clearIfUnchanged() }, RESUME_DELAY_MS)
    }
  }

  /**
   * Depuis Android 10, une application en arrière-plan ne peut pas lire le presse-papiers :
   * l'effacement attend alors son retour au premier plan.
   */
  private fun clearIfUnchanged() {
    val expected = copied ?: return
    if (!activity.hasWindowFocus()) {
      due = true
      return
    }
    copied = null
    due = false
    val current = clipboard.primaryClip?.takeIf { it.itemCount > 0 }?.getItemAt(0)?.text
    if (current != null && MessageDigest.isEqual(digest(current.toString()), expected)) {
      if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
        clipboard.clearPrimaryClip()
      } else {
        clipboard.setPrimaryClip(ClipData.newPlainText("", ""))
      }
    }
  }

  private fun digest(value: String): ByteArray =
    MessageDigest.getInstance("SHA-256").digest(value.toByteArray())

  private companion object {
    const val RESUME_DELAY_MS = 500L
  }
}
