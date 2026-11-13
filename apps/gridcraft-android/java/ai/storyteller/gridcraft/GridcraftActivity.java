package ai.storyteller.gridcraft;

import android.app.NativeActivity;
import android.content.Intent;
import android.content.ClipData;
import android.content.pm.PackageManager;
import android.database.Cursor;
import android.net.Uri;
import android.os.Build;
import android.os.Bundle;
import android.os.Environment;
import android.provider.Settings;
import android.provider.OpenableColumns;
import android.util.Log;
import android.webkit.MimeTypeMap;
import android.widget.Toast;
import java.io.File;
import java.io.FileOutputStream;
import java.io.InputStream;
import java.io.IOException;
import java.util.UUID;
import java.util.LinkedHashSet;
import java.util.ArrayList;
import java.util.concurrent.ConcurrentLinkedQueue;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;

/** Keep the native picker; bridge permission UI and incoming file-manager intents. */
public final class GridcraftActivity extends NativeActivity {
    private final ConcurrentLinkedQueue<String> openedDocuments = new ConcurrentLinkedQueue<>();
    private final ExecutorService imports = Executors.newSingleThreadExecutor();

    @Override public void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        receiveDocument(getIntent());
    }

    @Override protected void onNewIntent(Intent intent) {
        super.onNewIntent(intent);
        setIntent(intent);
        receiveDocument(intent);
    }

    /** Called from this Activity's native render thread, including after a cold start. */
    public String takeOpenedDocument() {
        return openedDocuments.poll();
    }

    private void receiveDocument(Intent intent) {
        if (intent == null) return;
        String action = intent.getAction();
        if (!Intent.ACTION_VIEW.equals(action) && !Intent.ACTION_EDIT.equals(action)
                && !Intent.ACTION_SEND.equals(action) && !Intent.ACTION_SEND_MULTIPLE.equals(action)) return;
        LinkedHashSet<Uri> uris = new LinkedHashSet<>();
        if (intent.getData() != null) uris.add(intent.getData());
        ClipData clip = intent.getClipData();
        if (clip != null) for (int i = 0; i < clip.getItemCount(); i++) {
            Uri uri = clip.getItemAt(i).getUri();
            if (uri != null) uris.add(uri);
        }
        if (Intent.ACTION_SEND.equals(action)) {
            Object stream = intent.getParcelableExtra(Intent.EXTRA_STREAM);
            if (stream instanceof Uri) uris.add((Uri) stream);
        } else if (Intent.ACTION_SEND_MULTIPLE.equals(action)) {
            ArrayList<Uri> streams = intent.getParcelableArrayListExtra(Intent.EXTRA_STREAM);
            if (streams != null) uris.addAll(streams);
        }
        final String mime = intent.getType();
        for (Uri uri : uris) {
            if (uri == null) continue;
            String scheme = uri.getScheme();
            if ("content".equalsIgnoreCase(scheme) || "file".equalsIgnoreCase(scheme)) {
                imports.execute(() -> importDocument(uri, mime));
            }
        }
    }

    private void importDocument(Uri uri, String mime) {
        File imported = null;
        try {
            if ("file".equals(uri.getScheme()) && uri.getPath() != null) {
                File original = new File(uri.getPath());
                if (original.isFile() && original.canRead()) {
                    openedDocuments.add(original.getAbsolutePath());
                    return;
                }
            }
            String name = uri.getLastPathSegment();
            try (Cursor cursor = getContentResolver().query(uri,
                    new String[]{OpenableColumns.DISPLAY_NAME}, null, null, null)) {
                if (cursor != null && cursor.moveToFirst() && !cursor.isNull(0)) name = cursor.getString(0);
            } catch (RuntimeException e) {
                Log.w("GridCraft", "No filename metadata for " + uri, e);
            }
            if (name == null || name.isEmpty()) name = "workbook";
            name = name.replace('/', '_').replace('\\', '_');
            if (name.equals(".") || name.equals("..")) name = "workbook";
            if (!name.contains(".")) {
                if (mime == null) mime = getContentResolver().getType(uri);
                String extension = MimeTypeMap.getSingleton().getExtensionFromMimeType(mime);
                if (extension != null && !extension.isEmpty()) name += "." + extension;
            }
            File dir = new File(getFilesDir(), "Imported/" + UUID.randomUUID());
            if (!dir.mkdirs()) throw new IOException("Cannot create import directory");
            imported = new File(dir, name);
            try (InputStream input = getContentResolver().openInputStream(uri);
                    FileOutputStream output = new FileOutputStream(imported)) {
                if (input == null) throw new IOException("Provider returned no stream");
                byte[] buffer = new byte[65536];
                int count;
                while ((count = input.read(buffer)) != -1) output.write(buffer, 0, count);
            }
            openedDocuments.add(imported.getAbsolutePath());
            runOnUiThread(() -> Toast.makeText(this, russian() ?
                "Получена копия файла. Для выбора папки сохранения используйте «Сохранить как»." :
                "Received a copy. Use Save As to choose where to save it.", Toast.LENGTH_LONG).show());
        } catch (Exception e) {
            if (imported != null) imported.delete();
            Log.e("GridCraft", "Cannot open incoming workbook " + uri, e);
            runOnUiThread(() -> Toast.makeText(this, russian() ? "Не удалось прочитать переданный файл" :
                "Cannot read the incoming file", Toast.LENGTH_LONG).show());
        }
    }

    /** read_dir may succeed with documents hidden by scoped storage; test the actual grant. */
    public boolean hasStorageAccess() {
        if (Build.VERSION.SDK_INT >= 30) return Environment.isExternalStorageManager();
        boolean read = checkSelfPermission("android.permission.READ_EXTERNAL_STORAGE") == PackageManager.PERMISSION_GRANTED;
        return read && (Build.VERSION.SDK_INT < 29 || Environment.isExternalStorageLegacy());
    }

    /** Runtime permission UI must run on Android's main thread, not its render thread. */
    public void requestStorageAccess() {
        runOnUiThread(() -> {
            if (Build.VERSION.SDK_INT < 30) {
                requestPermissions(new String[]{"android.permission.READ_EXTERNAL_STORAGE",
                    "android.permission.WRITE_EXTERNAL_STORAGE"}, 0x4752);
                return;
            }
            try {
                startActivity(new Intent(Settings.ACTION_MANAGE_APP_ALL_FILES_ACCESS_PERMISSION,
                    Uri.parse("package:" + getPackageName())));
            } catch (RuntimeException first) {
                try {
                    startActivity(new Intent(Settings.ACTION_MANAGE_ALL_FILES_ACCESS_PERMISSION));
                } catch (RuntimeException second) {
                    try {
                        startActivity(new Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS,
                            Uri.parse("package:" + getPackageName())));
                    } catch (RuntimeException last) {
                        Log.e("GridCraft", "Cannot open storage access settings", last);
                        Toast.makeText(this, russian() ? "Откройте настройки доступа к файлам для GridCraft" :
                            "Open file access settings for GridCraft", Toast.LENGTH_LONG).show();
                    }
                }
            }
        });
    }

    private boolean russian() {
        // This Russian distribution also localizes Android notices on an English OS.
        return true;
    }

    @Override protected void onDestroy() {
        imports.shutdown();
        super.onDestroy();
    }
}
