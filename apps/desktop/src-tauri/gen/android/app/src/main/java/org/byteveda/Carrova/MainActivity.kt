package org.byteveda.Carrova

import android.os.Bundle
import androidx.activity.enableEdgeToEdge
import org.byteveda.carrova.LinkService

class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
    // Links outlive the window: the service keeps the process (and Rust's supervisor) running.
    LinkService.start(this)
  }
}
