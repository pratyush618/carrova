package org.byteveda.carrova.platform

import android.os.Bundle
import androidx.activity.ComponentActivity
import com.journeyapps.barcodescanner.ScanContract
import com.journeyapps.barcodescanner.ScanOptions

/** Runs ZXing's camera screen (it asks for the camera permission) and hands its text to [Scanner]. */
class ScanActivity : ComponentActivity() {
    private val scan = registerForActivityResult(ScanContract()) { result ->
        Scanner.deliver(result.contents)
        finish()
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        if (savedInstanceState == null) {
            scan.launch(
                ScanOptions()
                    .setDesiredBarcodeFormats(ScanOptions.QR_CODE)
                    .setBeepEnabled(false)
                    .setOrientationLocked(false)
                    .setPrompt(""),
            )
        }
    }
}
