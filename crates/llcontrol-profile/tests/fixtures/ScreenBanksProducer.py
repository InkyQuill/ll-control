#!/usr/bin/env python3
"""Owned structural Screen DTO fixture; no vendor application or assets."""
import base64
import json
from pathlib import Path
import zipfile

root = Path(__file__).resolve().parent

def compact(value):
    return json.dumps(value, ensure_ascii=False, separators=(',', ':'))

runtime = compact({'Style':'Full','Screens':[{'Theme':{'Type':'Image','Path':'file:///C:/owned/uploads/Art.PNG','BackgroundColor':[1,2,3]},'Text':{'Type':'Custom','Text':'Owned runtime text','Layout':[{'FontFamily':'Owned Font','FontSize':43}]},'Watermark':{'Path':'','IsEnabled':False}}]})
slots = []
for i in range(8):
    slots.append({'ImageVideo':[compact({'ImageVideoPath':r'C:\owned\uploads\Art.PNG','TextCustom':{'Text':f'Owned editor {i}'},'TextThemeType':6})],
                  'BackgroundColor':[compact({'BackgroundColor':[i,10,20]})],
                  'Rain':['']})
profile = {'ScreenBrightness':64,'ScreenRotation':3,'ScreenConfigJson':runtime,
           'ScreenProfileName':['Owned current',None,'','Своя тема','Clock','5','6','7'],
           'ScreenEffectType':[1,2,3,1,17,1,1,1],'StyleExpressScreenEffectType':[19]*8,
           'ScreenProfiles':slots,
           'ScreenImageFileInfos':[{'FilePath':r'C:\owned\uploads\Art.PNG','IsDefaultFile':False},
                                   {'FilePath':r'C:\owned\default\Art.PNG','IsDefaultFile':True},
                                   {'FilePath':r'C:\owned\uploads\missing.jpg','IsDefaultFile':False}],
           'ScreenImageVideoFileInfos':[{'FilePath':r'C:\owned\uploads\motion.mp4','IsDefaultFile':False}],
           'ScreenWatermarkFileInfos':[{'FilePath':r'C:\owned\uploads\logo.png','IsDefaultFile':False}],
           'Fan':{'Mode':0}}
backup={'Version':'2.1.28.0','CreatedAt':None,'Type':1,'Datas':[{'MainType':0,'SubType':16846849,'Metadata':'DO-NOT-LEAK-SOURCE-IDENTITY','Data':base64.b64encode(compact(profile).encode()).decode()}]}
payloads = {'backup':compact(backup).encode(),'wmsq/art.png':b'owned image placeholder',
            'wmsq/motion.mp4':b'owned video placeholder','wmsq/logo.png':b'owned logo placeholder',
            'wmsq/unused.bin':b'owned unused upload','wmsq/art_s.jpg':b'owned thumbnail placeholder'}
with zipfile.ZipFile(root/'ScreenBanks.zip','w') as archive:
    for name, data in payloads.items():
        info=zipfile.ZipInfo(name,date_time=(2026,10,17,0,0,0))
        info.compress_type=zipfile.ZIP_DEFLATED
        archive.writestr(info,data)
