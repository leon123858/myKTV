import sys
import json
import yt_dlp
import os
import subprocess

def get_ffmpeg_path():
    """
    取得 ffmpeg 的執行路徑。
    區分三種場合:
    1. CD package (PyInstaller 打包的 frozen 執行檔): 使用與執行檔同目錄下的 ffmpeg，若無則 fallback
    2. local package / direct run: 直接調用環境變數中的 ffmpeg
    """
    if getattr(sys, 'frozen', False):
        base_dir = os.path.dirname(sys.executable)
        ffmpeg_exe_name = "ffmpeg.exe" if sys.platform == "win32" else "ffmpeg"
        ffmpeg_path = os.path.join(base_dir, ffmpeg_exe_name)
        if os.path.exists(ffmpeg_path):
            return ffmpeg_path
        else:
            return "ffmpeg"
    else:
        return "ffmpeg"

def my_hook(d):
    if d['status'] == 'downloading':
        # Send progress to stdout as JSON
        if d.get('total_bytes'):
            percent = d['downloaded_bytes'] / d['total_bytes'] * 100
        elif d.get('total_bytes_estimate'):
            percent = d['downloaded_bytes'] / d['total_bytes_estimate'] * 100
        else:
            percent = 0
            
        print(json.dumps({
            "status": "downloading",
            "percent": float(f"{percent:.2f}"),
            "filename": d.get('filename', '')
        }), flush=True)
    elif d['status'] == 'finished':
        print(json.dumps({
            "status": "finished",
            "filename": d.get('filename', '')
        }), flush=True)

def download_video(url, output_dir):
    data_dir = os.path.join(output_dir, "data")
    os.makedirs(data_dir, exist_ok=True)

    ydl_opts = {
        'format': 'bestvideo[ext=mp4]+bestaudio[ext=m4a]/best[ext=mp4]/best',
        'outtmpl': f'{output_dir}/%(title)s.%(ext)s',
        'progress_hooks': [my_hook],
        'quiet': True,
        'no_warnings': True,
        'postprocessors': [{
            'key': 'FFmpegVideoConvertor',
            'preferedformat': 'mp4',
        }],
        'ffmpeg_location': get_ffmpeg_path(),
    }

    try:
        with yt_dlp.YoutubeDL(ydl_opts) as ydl:
            print(json.dumps({"status": "starting", "url": url}), flush=True)
            info = ydl.extract_info(url, download=True)
            
            original_filename = ydl.prepare_filename(info)
            base_filename = os.path.splitext(original_filename)[0]
            final_mp4 = f"{base_filename}.mp4"
            
            if os.path.exists(final_mp4):
                title = os.path.basename(base_filename)
                video_out = os.path.join(data_dir, f"{title}_video.mp4")
                audio_out = os.path.join(data_dir, f"{title}_audio.mp3")
                
                print(json.dumps({"status": "processing", "message": "Splitting video and audio"}), flush=True)
                
                ffmpeg_bin = get_ffmpeg_path()
                subprocess.run([ffmpeg_bin, '-y', '-i', final_mp4, '-c:v', 'copy', '-an', video_out], 
                               stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
                subprocess.run([ffmpeg_bin, '-y', '-i', final_mp4, '-q:a', '2', '-vn', audio_out], 
                               stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            
            print(json.dumps({"status": "completed"}), flush=True)
    except Exception as e:
        print(json.dumps({"status": "error", "message": str(e)}), flush=True)

if __name__ == "__main__":
    if len(sys.argv) < 3:
        print(json.dumps({"status": "error", "message": "Usage: downloader <url> <output_dir>"}), flush=True)
        sys.exit(1)
        
    url = sys.argv[1]
    output_dir = sys.argv[2]
    download_video(url, output_dir)
