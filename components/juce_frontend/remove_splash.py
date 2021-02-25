#!/bin/env python

# JUCE allows removing the splash screen if your code is under GPLv3
path = '../../dependencies/juce/modules/juce_audio_processors/processors/juce_AudioProcessorEditor.cpp'
f = open(path, 'r', encoding='utf8')
content = f.read()
f.close()

start_label = '// BEGIN SECTION A'
end_label = '// END SECTION A'
start = content.find(start_label) + len(start_label)
end = content.find(end_label)
replace_with = '\n    // Audiobench is licensed under GPLv3'
replace_with += '\n    // splashScreen = new JUCESplashScreen (*this);'
replace_with += '\n    '
if content[start:end] == replace_with:
    print('Splash already removed.')
else:
    content = content[:start] + replace_with + content[end:]
    f = open(path, 'w', encoding='utf8')
    f.write(content)
    f.close()
