/** Annotation / comment host API. */
export {
  loadComments,
  createAnnotation,
  updateAnnotation,
  resolveAnnotation,
  reopenAnnotation,
  beginAcceptSuggestion,
  completeAcceptSuggestion,
  cancelAcceptSuggestion,
  getAcceptanceRecoveryStatus,
  rejectSuggestion,
  reconcileSessionAnnotations,
  type CommentDto,
  type AnnotationOpDto,
  type BeginAcceptDto,
  type AcceptanceRecoveryDto,
  type ReconcileDto,
} from "./index";
