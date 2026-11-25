package org.rerbr.mobile;

import android.app.Activity;
import android.app.Dialog;
import android.content.Context;
import android.security.keystore.KeyGenParameterSpec;
import android.security.keystore.KeyProperties;
import android.util.Base64;
import android.webkit.*;
import android.view.ViewGroup;
import org.json.JSONArray;
import org.json.JSONObject;
import java.io.*;
import java.nio.file.*;
import java.security.KeyStore;
import javax.crypto.*;
import javax.crypto.spec.GCMParameterSpec;

final class OnlineBoard {
    private final Activity parent;
    private final WebView web;
    private Dialog dialog;
    private boolean opened, ready;
    private static final String ORIGIN = "https://rbr.invalid/";
    OnlineBoard(Activity activity) {
        parent = activity;
        web = new WebView(activity);
        web.getSettings().setJavaScriptEnabled(true);
        web.getSettings().setAllowFileAccess(false);
        web.getSettings().setAllowContentAccess(false);
        web.addJavascriptInterface(new Bridge(),"RbrBoard");
        web.setWebViewClient(new WebViewClient() {
            @Override public boolean shouldOverrideUrlLoading(WebView v, WebResourceRequest r) { return true; }
            @Override public WebResourceResponse shouldInterceptRequest(WebView v, WebResourceRequest r) {
                String url = r.getUrl().toString();
                if (!url.startsWith(ORIGIN)) return null;
                String name = url.substring(ORIGIN.length());
                if (!name.matches("index\\.html|board\\.(css|js)|config\\.js")) return new WebResourceResponse("text/plain","UTF-8",null);
                String type = name.endsWith(".html") ? "text/html" : name.endsWith(".css") ? "text/css" : "text/javascript";
                try { return new WebResourceResponse(type,"UTF-8",parent.getAssets().open("leaderboard/"+name)); }
                catch(IOException e) { return new WebResourceResponse(type,"UTF-8",null); }
            }
            @Override public void onPageFinished(WebView v,String url) {
                ready=true;
                if(opened) web.evaluateJavascript("window.boardOpened()",null);
            }
        });
        web.loadUrl(ORIGIN+"index.html");
    }
    void show() {
        if(dialog != null) return;
        dialog=new Dialog(parent,android.R.style.Theme_Material_NoActionBar_Fullscreen);
        dialog.setContentView(web);
        dialog.setOnDismissListener(d -> {
            if(web.getParent() instanceof ViewGroup) ((ViewGroup)web.getParent()).removeView(web);
            opened=false; dialog=null;
        });
        dialog.show(); opened=true;
        if(ready) web.evaluateJavascript("window.boardOpened()",null);
    }
    void destroy() { if(dialog!=null) dialog.dismiss(); web.removeJavascriptInterface("RbrBoard"); web.destroy(); }
    private SecretKey sessionKey() throws Exception {
        KeyStore ks=KeyStore.getInstance("AndroidKeyStore"); ks.load(null);
        String alias="rbr-leaderboard-session-v1";
        if(ks.containsAlias(alias)) return (SecretKey)ks.getKey(alias,null);
        KeyGenerator gen=KeyGenerator.getInstance("AES","AndroidKeyStore");
        gen.init(new KeyGenParameterSpec.Builder(alias,KeyProperties.PURPOSE_ENCRYPT|KeyProperties.PURPOSE_DECRYPT)
            .setBlockModes(KeyProperties.BLOCK_MODE_GCM).setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE).build());
        return gen.generateKey();
    }
    private final class Bridge {
        @JavascriptInterface public String call(String op,String value) throws Exception {
            if(op.equals("close")) {parent.runOnUiThread(()->{if(dialog!=null) dialog.dismiss();});return "ok";}
            if(op.equals("session") || op.equals("persist")) {
                android.content.SharedPreferences prefs=parent.getSharedPreferences("board-v1",Context.MODE_PRIVATE);
                Cipher cipher=Cipher.getInstance("AES/GCM/NoPadding");
                if(op.equals("session")) {
                    String record=prefs.getString("session","");
                    if(record.isEmpty()) return "";
                    String[] parts=record.split(":",2);
                    cipher.init(Cipher.DECRYPT_MODE,sessionKey(),new GCMParameterSpec(128,Base64.decode(parts[0],0)));
                    return new String(cipher.doFinal(Base64.decode(parts[1],0)),java.nio.charset.StandardCharsets.UTF_8);
                }
                cipher.init(Cipher.ENCRYPT_MODE,sessionKey());
                String ciphertext=Base64.encodeToString(cipher.doFinal(value.getBytes("UTF-8")),Base64.NO_WRAP);
                return prefs.edit().putString("session",Base64.encodeToString(cipher.getIV(),Base64.NO_WRAP)+":"+ciphertext).commit()?"ok":"error";
            }
            Path root=parent.getExternalFilesDir(null).toPath().resolve("RBR/online-board");
            if(op.equals("selection")) return Files.exists(root.resolve("selection.json"))?new String(Files.readAllBytes(root.resolve("selection.json")),"UTF-8"):"{}";
            if(op.equals("pending")) {
                JSONArray list=new JSONArray();
                if(Files.isDirectory(root.resolve("pending"))) try(DirectoryStream<Path> files=Files.newDirectoryStream(root.resolve("pending"),"lb1-*.json")){
                    for(Path file:files) if(Files.size(file)<=8192) list.put(new JSONObject(new String(Files.readAllBytes(file),"UTF-8")));
                }
                return list.toString();
            }
            if(op.equals("ack") && value.matches("lb1-[0-9a-f]{20}")) {
                Files.createDirectories(root.resolve("sent"));
                Files.move(root.resolve("pending/"+value+".json"),root.resolve("sent/"+value+".json"),StandardCopyOption.REPLACE_EXISTING);
                return "ok";
            }
            throw new IllegalArgumentException("unknown board operation");
        }
    }
}
