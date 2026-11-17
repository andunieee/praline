package dev.praline.wallet

import android.app.NativeActivity
import android.content.Intent
import android.graphics.Color
import android.graphics.drawable.ColorDrawable
import android.os.Build
import android.os.Bundle
import android.view.SurfaceView
import android.view.View
import android.view.ViewGroup
import androidx.core.view.ViewCompat
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import com.google.zxing.integration.android.IntentIntegrator

private const val APP_BACKGROUND = 0xFF0D0D10.toInt()

class MainActivity : NativeActivity() {
    private fun findNativeSurfaceView(view: View): View? {
        if (view is SurfaceView) return view
        if (view is ViewGroup) {
            for (i in 0 until view.childCount) {
                val found = findNativeSurfaceView(view.getChildAt(i))
                if (found != null) return found
            }
        }
        return null
    }

    @Suppress("DEPRECATION")
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        // NativeActivity loads the library with dlopen, which the JVM does not
        // know about. Register it so `nativeSetInsets` can be resolved.
        System.loadLibrary("praline")

        // Edge-to-edge. NativeActivity hands the native window the whole
        // surface, so the insets are sent to Rust and applied as UI padding.
        window.setBackgroundDrawable(ColorDrawable(APP_BACKGROUND))
        WindowCompat.setDecorFitsSystemWindows(window, false)
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.VANILLA_ICE_CREAM) {
            window.statusBarColor = Color.TRANSPARENT
            window.navigationBarColor = Color.TRANSPARENT
        }
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            window.isStatusBarContrastEnforced = false
            window.isNavigationBarContrastEnforced = false
        }

        ViewCompat.setOnApplyWindowInsetsListener(window.decorView) { _, insets ->
            val density = resources.displayMetrics.density
            val bars = insets.getInsets(
                WindowInsetsCompat.Type.systemBars() or
                    WindowInsetsCompat.Type.displayCutout()
            )
            val ime = insets.getInsets(WindowInsetsCompat.Type.ime()).bottom
            nativeSetInsets(
                bars.left / density,
                bars.top / density,
                bars.right / density,
                maxOf(bars.bottom, ime) / density,
            )
            insets
        }

        window.decorView.post {
            findNativeSurfaceView(window.decorView)?.apply {
                isFocusable = true
                isFocusableInTouchMode = true
                requestFocus()
            }
        }
    }

    private external fun nativeSetInsets(left: Float, top: Float, right: Float, bottom: Float)

    /** Opens the embedded ZXing scanner. Called from Rust on the Java main thread. */
    @Suppress("DEPRECATION")
    fun startScanner() {
        IntentIntegrator(this)
            .setDesiredBarcodeFormats(IntentIntegrator.QR_CODE)
            .setPrompt("Scan a Lightning invoice")
            .setBeepEnabled(false)
            .setOrientationLocked(false)
            .initiateScan()
    }

    @Suppress("DEPRECATION")
    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        super.onActivityResult(requestCode, resultCode, data)

        val result = IntentIntegrator.parseActivityResult(requestCode, resultCode, data) ?: return
        result.contents
            ?.takeIf { it.isNotBlank() }
            ?.let(::nativeOnQrScanned)
    }

    private external fun nativeOnQrScanned(contents: String)
}
