package com.mctx.app

import android.app.Activity
import android.content.Intent
import android.net.Uri
import android.os.Bundle
import android.provider.OpenableColumns
import android.view.inputmethod.EditorInfo
import android.widget.Button
import android.widget.EditText
import android.widget.TextView
import android.widget.Toast
import java.io.IOException

/**
 * A minimal notepad for .mctx memory files using Android's Storage Access
 * Framework: open any text file (SAF), edit it, save it. .mctx files are
 * plain text, so this works for memory files opened from any provider
 * (Downloads, Drive, Termux home, etc.).
 */
class MainActivity : Activity() {

    private lateinit var editor: EditText
    private var currentUri: Uri? = null

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_main)

        editor = findViewById(R.id.editor)
        editor.setImeOptions(editor.imeOptions or EditorInfo.IME_FLAG_NO_ENTER_ACTION)

        findViewById<Button>(R.id.openBtn).setOnClickListener {
            val intent = Intent(Intent.ACTION_OPEN_DOCUMENT).apply {
                addCategory(Intent.CATEGORY_OPENABLE)
                type = "*/*"
                addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_WRITE_URI_PERMISSION)
            }
            startActivityForResult(intent, REQ_OPEN)
        }

        findViewById<Button>(R.id.saveBtn).setOnClickListener {
            val uri = currentUri
            if (uri != null) {
                saveTo(uri)
            } else {
                val intent = Intent(Intent.ACTION_CREATE_DOCUMENT).apply {
                    addCategory(Intent.CATEGORY_OPENABLE)
                    type = "*/*"
                    putExtra(Intent.EXTRA_TITLE, "memory.mctx")
                }
                startActivityForResult(intent, REQ_CREATE)
            }
        }

        // Allow opening an .mctx from another app (share / file manager).
        handleViewIntent(intent)
    }

    private fun handleViewIntent(intent: Intent?) {
        val data = intent?.data ?: return
        openUri(data)
    }

    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        super.onActivityResult(requestCode, resultCode, data)
        val uri = data?.data ?: return
        if (resultCode != RESULT_OK) return
        when (requestCode) {
            REQ_OPEN -> openUri(uri)
            REQ_CREATE -> {
                currentUri = uri
                saveTo(uri)
            }
        }
    }

    private fun openUri(uri: Uri) {
        try {
            contentResolver
                .openInputStream(uri)!!
                .bufferedReader()
                .use { editor.setText(it.readText()) }
            currentUri = uri
            findViewById<TextView>(R.id.title).text = displayName(uri)
            Toast.makeText(this, "opened", Toast.LENGTH_SHORT).show()
        } catch (e: IOException) {
            Toast.makeText(this, "open failed: ${e.message}", Toast.LENGTH_LONG).show()
        }
    }

    private fun saveTo(uri: Uri) {
        try {
            contentResolver
                .openOutputStream(uri, "wt")!!
                .writer()
                .use { it.write(editor.text.toString()) }
            Toast.makeText(this, "saved", Toast.LENGTH_SHORT).show()
        } catch (e: IOException) {
            Toast.makeText(this, "save failed: ${e.message}", Toast.LENGTH_LONG).show()
        }
    }

    private fun displayName(uri: Uri): String {
        contentResolver.query(uri, null, null, null, null)?.use { c ->
            val idx = c.getColumnIndex(OpenableColumns.DISPLAY_NAME)
            if (idx >= 0 && c.moveToFirst()) return c.getString(idx) ?: "memory.mctx"
        }
        return "memory.mctx"
    }

    companion object {
        private const val REQ_OPEN = 1
        private const val REQ_CREATE = 2
    }
}
