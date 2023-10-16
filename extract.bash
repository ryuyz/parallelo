OUT_GSURI='gs://fvital-sandbox-bucket/ncchd-asd/yolo-outs/mp4-separated/'

list-tar () {
  gsutil ls 'gs://fvital-sandbox-bucket/ncchd-asd/yolo-outs/archived/*.tar'
}

abspath-to-stem () {
  sed -r 's/(.*)\.tar/\1/'
}

download-from-gsuri-and-print-local-abspath () {
  local GSURI
  local BASENAME
  read -r GSURI
  BASENAME=$(basename ${GSURI})
  mkdir -p ./tmp
  gsutil cp ${GSURI} ./tmp/${BASENAME} >&2
  realpath ./tmp/${BASENAME}
}

extract-all () {
  local GSURI
  while read -r GSURI; do
    echo ${GSURI} | extract
  done
}

# arg: gsuri
extract () {
  local GSURI
  read -r GSURI

  local TAR_ABSPATH=$(
    echo ${GSURI} \
      | download-from-gsuri-and-print-local-abspath
  )
  echo TAR_ABSPATH=${TAR_ABSPATH} >&2

  tar xf ${TAR_ABSPATH} -C ./tmp

  local DIR_ABSPATH=$(
    echo ${TAR_ABSPATH} | abspath-to-stem
  )
  echo DIR_ABSPATH=${DIR_ABSPATH} >&2

  local STEM=$(basename ${DIR_ABSPATH})
  echo STEM=${STEM} >&2

  local MP4_ABSPATH=$(
    find ${DIR_ABSPATH} -name '*.mp4'
  )
  echo MP4_ABSPATH=${MP4_ABSPATH} >&2


  gsutil cp ${MP4_ABSPATH} ${OUT_GSURI}
  rm -f ${MP4_ABSPATH}

  local LABELS_TAR_ABSPATH=${DIR_ABSPATH}.labels.tar

  tar cvf ${LABELS_TAR_ABSPATH} -C ./tmp ${STEM}

  rm -rf ${DIR_ABSPATH}

  gsutil cp ${LABELS_TAR_ABSPATH} ${OUT_GSURI}
}

main () {
  rm -rf ./tmp
  list-tar | extract-all
}

main
