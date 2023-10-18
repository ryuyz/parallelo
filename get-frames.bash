#!/usr/bin/env bash

# $1 is the input video file uri

tmp_name=./tmp-get-frames

mkdir -p ${tmp_name}
gsutil cp $1 ${tmp_name}/

# Path to the input video file
input_file="${tmp_name}/$(basename $1)"
frame_count=20

out_dirname="${input_file%.*}"

mkdir -p ${out_dirname}

# Output file pattern
output_pattern="${out_dirname}/%02d_%s.jpg"

# Get the total duration of the video in seconds
total_duration=$(ffprobe -i "${input_file}" -show_entries format=duration -v quiet -of csv="p=0")
total_duration="${total_duration%.*}"  # Remove decimal part to get an integer

# Loop to randomly pick 1000 frames
for i in $(seq 1 "${frame_count}"); do
  # Generate a random time (in seconds)
  random_time=$(shuf -i "0-${total_duration}" -n 1)
  
  # Convert random time to HH:MM:SS format
  hh=$(printf "%02d" $((random_time / 3600)))
  mm=$(printf "%02d" $(( (random_time % 3600) / 60 )))
  ss=$(printf "%02d" $((random_time % 60)))
  time_str="${hh}:${mm}:${ss}"

  # Extract the frame at the random time
  ffmpeg -ss ${time_str} -i ${input_file} -vframes 1 -q:v 2 $(printf ${output_pattern} ${i} ${time_str} )
done

