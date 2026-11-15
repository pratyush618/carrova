package org.byteveda.Carrova

import android.content.res.Configuration
import android.graphics.drawable.ColorDrawable
import android.os.Bundle
import android.view.View
import androidx.activity.enableEdgeToEdge
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat

class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    // Transparent bars with icons that follow light/dark; the window colour shows behind them.
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
    paintWindow()
    keepClearOfSystemBars(findViewById(android.R.id.content))
  }

  // `uiMode` is in configChanges: a light/dark switch doesn't recreate the activity.
  override fun onConfigurationChanged(newConfig: Configuration) {
    super.onConfigurationChanged(newConfig)
    enableEdgeToEdge()
    paintWindow()
  }

  private fun paintWindow() {
    window.setBackgroundDrawable(ColorDrawable(getColor(R.color.window_background)))
  }

  /**
   * Edge-to-edge draws the WebView under the status bar, cutout, nav bar and
   * keyboard; pad the content (whatever view Rust puts in it) by those insets.
   */
  private fun keepClearOfSystemBars(content: View) {
    val types = WindowInsetsCompat.Type.systemBars() or
      WindowInsetsCompat.Type.displayCutout() or
      WindowInsetsCompat.Type.ime()
    ViewCompat.setOnApplyWindowInsetsListener(content) { view, insets ->
      val bars = insets.getInsets(types)
      view.setPadding(bars.left, bars.top, bars.right, bars.bottom)
      WindowInsetsCompat.CONSUMED
    }
  }
}
