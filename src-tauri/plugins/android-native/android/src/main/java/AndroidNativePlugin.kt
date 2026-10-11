package app.moodledesk.androidnative

import android.app.Activity
import android.content.ActivityNotFoundException
import android.content.Context
import android.content.Intent
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import android.webkit.MimeTypeMap
import androidx.core.content.FileProvider
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import java.io.File
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

@InvokeArg
class PathArg {
    lateinit var path: String
}

@InvokeArg
class ValueArg {
    lateinit var value: String
}

@TauriPlugin
class AndroidNativePlugin(private val activity: Activity) : Plugin(activity) {
    private val prefs get() = activity.getSharedPreferences("moodledesk_secrets", Context.MODE_PRIVATE)

    /** Opens a downloaded file with the app the user picks. */
    @Command
    fun openFile(invoke: Invoke) {
        try {
            val source = File(invoke.parseArgs(PathArg::class.java).path)
            // the provider only serves the cache folder, so the app gets a copy of the file from there
            // shortcut: copies stay in the cache until Android clears it, delete old ones if big files pile up
            val copy = File(File(activity.cacheDir, "open").apply { mkdirs() }, source.name)
            source.copyTo(copy, overwrite = true)
            val uri = FileProvider.getUriForFile(activity, "${activity.packageName}.openfile", copy)
            val mime = MimeTypeMap.getSingleton().getMimeTypeFromExtension(copy.extension.lowercase())
                ?: "application/octet-stream"
            activity.startActivity(
                Intent(Intent.ACTION_VIEW).setDataAndType(uri, mime).addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
            )
            invoke.resolve()
        } catch (e: ActivityNotFoundException) {
            invoke.reject("No app on this phone can open this file")
        } catch (e: Exception) {
            invoke.reject(e.message ?: "Could not open the file")
        }
    }

    /** The key is made once and stays inside the Android Keystore: this code only ever asks it to encrypt and decrypt. */
    private fun key(): SecretKey {
        val keystore = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        (keystore.getKey(KEY_ALIAS, null) as? SecretKey)?.let { return it }
        return KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, "AndroidKeyStore").run {
            init(
                KeyGenParameterSpec.Builder(KEY_ALIAS, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
                    .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                    .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                    .setKeySize(256)
                    .build()
            )
            generateKey()
        }
    }

    @Command
    fun secretSet(invoke: Invoke) {
        try {
            val cipher = Cipher.getInstance(CIPHER).apply { init(Cipher.ENCRYPT_MODE, key()) }
            val sealed = cipher.iv + cipher.doFinal(invoke.parseArgs(ValueArg::class.java).value.toByteArray())
            prefs.edit().putString(PREF, Base64.encodeToString(sealed, Base64.NO_WRAP)).commit()
            invoke.resolve()
        } catch (e: Exception) {
            invoke.reject(e.message ?: "Could not save the login")
        }
    }

    @Command
    fun secretGet(invoke: Invoke) {
        val result = JSObject()
        try {
            val sealed = Base64.decode(prefs.getString(PREF, null) ?: return invoke.resolve(result), Base64.NO_WRAP)
            val cipher = Cipher.getInstance(CIPHER)
                .apply { init(Cipher.DECRYPT_MODE, key(), GCMParameterSpec(128, sealed, 0, IV_SIZE)) }
            result.put("value", String(cipher.doFinal(sealed, IV_SIZE, sealed.size - IV_SIZE)))
        } catch (e: Exception) {
            // key gone (a restored backup, a reset lock screen): same as not being logged in
        }
        invoke.resolve(result)
    }

    @Command
    fun secretDelete(invoke: Invoke) {
        prefs.edit().remove(PREF).commit()
        invoke.resolve()
    }

    private companion object {
        const val KEY_ALIAS = "moodledesk_session"
        const val CIPHER = "AES/GCM/NoPadding"
        const val PREF = "session"
        const val IV_SIZE = 12
    }
}
